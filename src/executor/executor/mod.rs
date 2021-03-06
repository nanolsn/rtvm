#[cfg(test)]
mod tests;

use super::{
    files::{Files, FilesError},
    memory::*,
    primary::*,
};
use crate::common::*;

#[derive(Debug)]
pub struct Function<'f> {
    frame_size: UWord,
    program: &'f [Op],
}

#[derive(Debug)]
pub struct FunctionCall<'f> {
    function: &'f Function<'f>,
    base_ptr: UWord,
    ret_val_ptr: UWord,
    ret_program_counter: UWord,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    EndOfProgram,
    MemoryError(MemoryError),
    FilesError(FilesError),
    IncorrectOperation(Op),
    UnknownFunction(UWord),
    OperationOverflow,
    DivisionByZero,
    NullPointerDereference,
}

impl From<MemoryError> for ExecutionError {
    fn from(e: MemoryError) -> Self {
        ExecutionError::MemoryError(e)
    }
}

impl From<FilesError> for ExecutionError {
    fn from(e: FilesError) -> Self {
        ExecutionError::FilesError(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExecutionSuccess {
    Ok,
    End(UWord),
    Sleep(UWord),
}

pub type Executed = Result<ExecutionSuccess, ExecutionError>;

#[derive(Debug)]
pub struct Executor<'f> {
    functions: &'f [Function<'f>],
    memory: Memory,
    program_counter: UWord,
    call_stack: Vec<FunctionCall<'f>>,
    prepared_call: bool,
    parameter_ptr: UWord,
    files: Files,
}

macro_rules! impl_cnv {
    ($t:ty, $obj:ident, $uid:ident, $x:ident, $y:ident) => {
        match $uid {
            U8 => $obj.exec_cnv::<$t, u8>($x, $y)?,
            I8 => $obj.exec_cnv::<$t, i8>($x, $y)?,
            U16 => $obj.exec_cnv::<$t, u16>($x, $y)?,
            I16 => $obj.exec_cnv::<$t, i16>($x, $y)?,
            U32 => $obj.exec_cnv::<$t, u32>($x, $y)?,
            I32 => $obj.exec_cnv::<$t, i32>($x, $y)?,
            U64 => $obj.exec_cnv::<$t, u64>($x, $y)?,
            I64 => $obj.exec_cnv::<$t, i64>($x, $y)?,
            Uw => $obj.exec_cnv::<$t, UWord>($x, $y)?,
            Iw => $obj.exec_cnv::<$t, IWord>($x, $y)?,
            F32 => $obj.exec_cnv::<$t, f32>($x, $y)?,
            F64 => $obj.exec_cnv::<$t, f64>($x, $y)?,
        }
    };
}

impl<'f> Executor<'f> {
    pub fn new(functions: &'f [Function]) -> Self {
        const STACK_LIMIT: usize = 2048;
        const HEAP_LIMIT: usize = 2048;

        Self::from_limits(functions, STACK_LIMIT, HEAP_LIMIT)
    }

    pub fn from_limits(functions: &'f [Function], stack_limit: usize, heap_limit: usize) -> Self {
        Self {
            functions,
            memory: Memory::from_limits(stack_limit, heap_limit),
            program_counter: 0,
            call_stack: Vec::new(),
            prepared_call: false,
            parameter_ptr: 0,
            files: Files::new(),
        }
    }

    fn app(&mut self, function_id: UWord) -> Result<(), ExecutionError> {
        let f = self
            .functions
            .get(function_id as usize)
            .ok_or(ExecutionError::UnknownFunction(function_id))?;

        self.call_stack.push(FunctionCall {
            function: f,
            base_ptr: self.memory.stack.len(),
            ret_val_ptr: 0,
            ret_program_counter: 0,
        });

        self.prepared_call = true;
        self.memory.stack.expand(f.frame_size)?;

        Ok(())
    }

    fn clf(&mut self, ret_val_ptr: UWord) -> Result<(), ExecutionError> {
        let current_fn = self
            .call_stack
            .last_mut()
            .ok_or(ExecutionError::EndOfProgram)?;

        current_fn.ret_val_ptr = ret_val_ptr;
        current_fn.ret_program_counter = self.program_counter.wrapping_add(1);
        self.prepared_call = false;
        self.program_counter = 0;
        self.parameter_ptr = 0;

        Ok(())
    }

    pub fn call(&mut self, function_id: UWord, ret_val_ptr: UWord) -> Result<(), ExecutionError> {
        self.app(function_id)?;
        self.clf(ret_val_ptr)
    }

    fn ret(&mut self) -> Result<(), ExecutionError> {
        let current_fn = self.call_stack.pop().ok_or(ExecutionError::EndOfProgram)?;

        self.program_counter = current_fn.ret_program_counter;
        self.memory.stack.narrow(current_fn.function.frame_size)?;

        Ok(())
    }

    fn current_call(&self) -> Result<&FunctionCall, ExecutionError> {
        let call = if self.prepared_call {
            self.call_stack.get(self.call_stack.len().wrapping_sub(2))
        } else {
            self.call_stack.last()
        };

        call.ok_or(ExecutionError::EndOfProgram)
    }

    fn current_op(&self) -> Result<&Op, ExecutionError> {
        self.current_call()?
            .function
            .program
            .get(self.program_counter as usize)
            .ok_or(ExecutionError::EndOfProgram)
    }

    fn pass_condition(&mut self) -> Result<(), ExecutionError> {
        loop {
            self.program_counter += 1;
            let op = self.current_op()?;

            if !op.is_conditional() {
                self.program_counter += 1;
                break Ok(());
            }
        }
    }

    fn get_val<T>(&self, operand: Operand) -> Result<T, ExecutionError>
    where
        T: Primary,
    {
        Ok(match operand {
            Operand::Loc(loc) => self
                .memory
                .get(self.current_call()?.base_ptr.wrapping_add(loc))?,
            Operand::Ind(ptr) => {
                if ptr == 0 {
                    return Err(ExecutionError::NullPointerDereference);
                } else {
                    self.memory.get(
                        self.memory
                            .get(self.current_call()?.base_ptr.wrapping_add(ptr))?,
                    )?
                }
            }
            Operand::Ret(ret) => self
                .memory
                .get(self.current_call()?.ret_val_ptr.wrapping_add(ret))?,
            Operand::Val(val) => T::from_word(val),
            Operand::Ref(var) => T::from_word(self.current_call()?.base_ptr.wrapping_add(var)),
            Operand::Glb(ptr) => self.memory.get(ptr)?,
            Operand::Emp => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
        })
    }

    fn set_val<T>(&mut self, operand: Operand, val: T) -> Result<(), ExecutionError>
    where
        T: Primary,
    {
        Ok(match operand {
            Operand::Loc(loc) => self
                .memory
                .set(self.current_call()?.base_ptr.wrapping_add(loc), val)?,
            Operand::Ind(ptr) => {
                if ptr == 0 {
                    return Err(ExecutionError::NullPointerDereference);
                } else {
                    self.memory.set(
                        self.memory
                            .get(self.current_call()?.base_ptr.wrapping_add(ptr))?,
                        val,
                    )?
                }
            }
            Operand::Ret(ret) => self
                .memory
                .set(self.current_call()?.ret_val_ptr.wrapping_add(ret), val)?,
            Operand::Val(_) => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
            Operand::Ref(_) => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
            Operand::Glb(ptr) => self.memory.set(ptr, val)?,
            Operand::Emp => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
        })
    }

    fn read_un_operand(&self, un: UnOp) -> Result<Operand, ExecutionError> {
        Ok(match un {
            UnOp::None { x } => x,
            UnOp::First { x, offset } => self.make_offset(x, offset)?,
        })
    }

    fn read_bin_operands(&self, bin: BinOp) -> Result<(Operand, Operand), ExecutionError> {
        Ok(match bin {
            BinOp::None { x, y } => (x, y),
            BinOp::First { x, y, offset } => (self.make_offset(x, offset)?, y),
            BinOp::Second { x, y, offset } => (x, self.make_offset(y, offset)?),
            BinOp::Both { x, y, offset } => {
                (self.make_offset(x, offset)?, self.make_offset(y, offset)?)
            }
        })
    }

    fn get_un<T>(&mut self, un: UnOp) -> Result<T, ExecutionError>
    where
        T: Primary,
    {
        let left = self.read_un_operand(un)?;
        self.get_val(left)
    }

    fn update_un<T, U, F>(&mut self, un: UnOp, f: F) -> Result<(), ExecutionError>
    where
        T: Primary,
        U: Primary,
        F: FnOnce(T) -> U,
    {
        let left = self.read_un_operand(un)?;
        self.set_val(left, f(self.get_val(left)?))
    }

    fn update_bin<T, U, F>(&mut self, bin: BinOp, f: F) -> Result<(), ExecutionError>
    where
        T: Primary,
        U: Primary,
        F: FnOnce(T, T) -> U,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        self.set_val(left, f(self.get_val(left)?, self.get_val(right)?))
    }

    fn update_bin_division<T, F>(&mut self, bin: BinOp, f: F) -> Result<(), ExecutionError>
    where
        T: Primary + PartialEq,
        F: FnOnce(T, T) -> T,
    {
        let mut div_by_zero = false;

        let res = self.update_bin::<T, T, _>(bin, |x, y| {
            if y == T::zero() {
                div_by_zero = true;
                T::zero()
            } else {
                f(x, y)
            }
        });

        if div_by_zero {
            return Err(ExecutionError::DivisionByZero);
        }

        res
    }

    fn make_offset(&self, a: Operand, offset: Operand) -> Result<Operand, ExecutionError> {
        let a_offset: UWord = self.get_val(offset)?;
        Ok(a.map(|a| a.wrapping_add(a_offset)))
    }

    fn exec_set<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Primary,
    {
        self.update_bin::<T, T, _>(bin, |_, y| y)
    }

    fn exec_cnv<T, U>(&mut self, left: Operand, right: Operand) -> Result<(), ExecutionError>
    where
        T: Primary,
        U: Convert<T>,
    {
        self.set_val(left, U::convert(self.get_val(right)?))
    }

    fn exec_add<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Add,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x.wrapping(y))
    }

    fn exec_sub<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Sub,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x.wrapping(y))
    }

    fn exec_mul<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Mul,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x.wrapping(y))
    }

    fn exec_div<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Div + PartialEq,
    {
        self.update_bin_division::<T, _>(bin, |x, y| x.wrapping(y))
    }

    fn exec_mod<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Rem + PartialEq,
    {
        self.update_bin_division::<T, _>(bin, |x, y| x.wrapping(y))
    }

    fn exec_shl<T>(&mut self, x: Operand, y: Operand) -> Result<(), ExecutionError>
    where
        T: Shl,
    {
        let x_val: T = self.get_val(x)?;
        let y_val: u8 = self.get_val(y)?;
        self.set_val(x, x_val.wrapping(y_val))
    }

    fn exec_shr<T>(&mut self, x: Operand, y: Operand) -> Result<(), ExecutionError>
    where
        T: Shr,
    {
        let x_val: T = self.get_val(x)?;
        let y_val: u8 = self.get_val(y)?;
        self.set_val(x, x_val.wrapping(y_val))
    }

    fn exec_and<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Primary + std::ops::BitAnd<Output = T>,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x & y)
    }

    fn exec_or<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Primary + std::ops::BitOr<Output = T>,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x | y)
    }

    fn exec_xor<T>(&mut self, bin: BinOp) -> Result<(), ExecutionError>
    where
        T: Primary + std::ops::BitXor<Output = T>,
    {
        self.update_bin::<T, T, _>(bin, |x, y| x ^ y)
    }

    fn exec_not<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Primary + std::ops::Not<Output = T>,
    {
        self.update_un::<T, T, _>(un, |y| !y)
    }

    fn exec_neg<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Neg,
    {
        self.update_un::<T, T, _>(un, |x| x.wrapping())
    }

    fn exec_inc<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Inc,
    {
        self.update_un::<T, T, _>(un, |x| x.wrapping())
    }

    fn exec_dec<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Dec,
    {
        self.update_un::<T, T, _>(un, |x| x.wrapping())
    }

    fn exec_ife<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? == self.get_val::<T>(right)?)
    }

    fn exec_ifl<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialOrd,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? < self.get_val::<T>(right)?)
    }

    fn exec_ifg<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialOrd,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? > self.get_val::<T>(right)?)
    }

    fn exec_ine<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? != self.get_val::<T>(right)?)
    }

    fn exec_inl<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialOrd,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? >= self.get_val::<T>(right)?)
    }

    fn exec_ing<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialOrd,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? <= self.get_val::<T>(right)?)
    }

    fn exec_ifa<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitAnd<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? & self.get_val::<T>(right)? != T::zero())
    }

    fn exec_ifo<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitOr<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? | self.get_val::<T>(right)? != T::zero())
    }

    fn exec_ifx<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitXor<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? ^ self.get_val::<T>(right)? != T::zero())
    }

    fn exec_ina<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitAnd<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? & self.get_val::<T>(right)? == T::zero())
    }

    fn exec_ino<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitOr<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? | self.get_val::<T>(right)? == T::zero())
    }

    fn exec_inx<T>(&self, bin: BinOp) -> Result<bool, ExecutionError>
    where
        T: Primary + PartialEq + std::ops::BitXor<Output = T>,
    {
        let (left, right) = self.read_bin_operands(bin)?;
        Ok(self.get_val::<T>(left)? ^ self.get_val::<T>(right)? == T::zero())
    }

    fn exec_par<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Primary,
    {
        let frame_size = self.current_call()?.function.frame_size;
        let parameter_loc = self.parameter_ptr.wrapping_add(frame_size);
        self.parameter_ptr = self.parameter_ptr.wrapping_add(T::SIZE as UWord);

        let val = self.get_un(un)?;
        self.set_val::<T>(Operand::Loc(parameter_loc), val)?;

        Ok(())
    }

    fn set_ret<T>(&mut self, un: UnOp) -> Result<(), ExecutionError>
    where
        T: Primary,
    {
        let right = self.read_un_operand(un)?;
        self.set_val::<T>(Operand::Ret(0), self.get_val(right)?)
    }

    pub fn execute(&mut self) -> Executed {
        use Op::*;
        use OpType::*;

        let &op = self.current_op()?;

        let res = match op {
            Nop => Ok(ExecutionSuccess::Ok),
            End(x) => {
                let val = self.get_val(x)?;
                Ok(ExecutionSuccess::End(val))
            }
            Slp(x) => {
                let val = self.get_val(x)?;
                Ok(ExecutionSuccess::Sleep(val))
            }
            Set(bin, ot) => {
                match ot {
                    U8 => self.exec_set::<u8>(bin)?,
                    I8 => self.exec_set::<i8>(bin)?,
                    U16 => self.exec_set::<u16>(bin)?,
                    I16 => self.exec_set::<i16>(bin)?,
                    U32 => self.exec_set::<u32>(bin)?,
                    I32 => self.exec_set::<i32>(bin)?,
                    U64 => self.exec_set::<u64>(bin)?,
                    I64 => self.exec_set::<i64>(bin)?,
                    Uw => self.exec_set::<UWord>(bin)?,
                    Iw => self.exec_set::<IWord>(bin)?,
                    F32 => self.exec_set::<f32>(bin)?,
                    F64 => self.exec_set::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Cnv(x, y, t, u) => {
                match t {
                    U8 => impl_cnv!(u8, self, u, x, y),
                    I8 => impl_cnv!(i8, self, u, x, y),
                    U16 => impl_cnv!(u16, self, u, x, y),
                    I16 => impl_cnv!(i16, self, u, x, y),
                    U32 => impl_cnv!(u32, self, u, x, y),
                    I32 => impl_cnv!(i32, self, u, x, y),
                    U64 => impl_cnv!(u64, self, u, x, y),
                    I64 => impl_cnv!(i64, self, u, x, y),
                    Uw => impl_cnv!(UWord, self, u, x, y),
                    Iw => impl_cnv!(IWord, self, u, x, y),
                    F32 => impl_cnv!(f32, self, u, x, y),
                    F64 => impl_cnv!(f64, self, u, x, y),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Add(bin, ot) => {
                match ot {
                    U8 => self.exec_add::<u8>(bin)?,
                    I8 => self.exec_add::<i8>(bin)?,
                    U16 => self.exec_add::<u16>(bin)?,
                    I16 => self.exec_add::<i16>(bin)?,
                    U32 => self.exec_add::<u32>(bin)?,
                    I32 => self.exec_add::<i32>(bin)?,
                    U64 => self.exec_add::<u64>(bin)?,
                    I64 => self.exec_add::<i64>(bin)?,
                    Uw => self.exec_add::<UWord>(bin)?,
                    Iw => self.exec_add::<IWord>(bin)?,
                    F32 => self.exec_add::<f32>(bin)?,
                    F64 => self.exec_add::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Sub(bin, ot) => {
                match ot {
                    U8 => self.exec_sub::<u8>(bin)?,
                    I8 => self.exec_sub::<i8>(bin)?,
                    U16 => self.exec_sub::<u16>(bin)?,
                    I16 => self.exec_sub::<i16>(bin)?,
                    U32 => self.exec_sub::<u32>(bin)?,
                    I32 => self.exec_sub::<i32>(bin)?,
                    U64 => self.exec_sub::<u64>(bin)?,
                    I64 => self.exec_sub::<i64>(bin)?,
                    Uw => self.exec_sub::<UWord>(bin)?,
                    Iw => self.exec_sub::<IWord>(bin)?,
                    F32 => self.exec_sub::<f32>(bin)?,
                    F64 => self.exec_sub::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Mul(bin, ot) => {
                match ot {
                    U8 => self.exec_mul::<u8>(bin)?,
                    I8 => self.exec_mul::<i8>(bin)?,
                    U16 => self.exec_mul::<u16>(bin)?,
                    I16 => self.exec_mul::<i16>(bin)?,
                    U32 => self.exec_mul::<u32>(bin)?,
                    I32 => self.exec_mul::<i32>(bin)?,
                    U64 => self.exec_mul::<u64>(bin)?,
                    I64 => self.exec_mul::<i64>(bin)?,
                    Uw => self.exec_mul::<UWord>(bin)?,
                    Iw => self.exec_mul::<IWord>(bin)?,
                    F32 => self.exec_mul::<f32>(bin)?,
                    F64 => self.exec_mul::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Div(bin, ot) => {
                match ot {
                    U8 => self.exec_div::<u8>(bin)?,
                    I8 => self.exec_div::<i8>(bin)?,
                    U16 => self.exec_div::<u16>(bin)?,
                    I16 => self.exec_div::<i16>(bin)?,
                    U32 => self.exec_div::<u32>(bin)?,
                    I32 => self.exec_div::<i32>(bin)?,
                    U64 => self.exec_div::<u64>(bin)?,
                    I64 => self.exec_div::<i64>(bin)?,
                    Uw => self.exec_div::<UWord>(bin)?,
                    Iw => self.exec_div::<IWord>(bin)?,
                    F32 => self.exec_div::<f32>(bin)?,
                    F64 => self.exec_div::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Mod(bin, ot) => {
                match ot {
                    U8 => self.exec_mod::<u8>(bin)?,
                    I8 => self.exec_mod::<i8>(bin)?,
                    U16 => self.exec_mod::<u16>(bin)?,
                    I16 => self.exec_mod::<i16>(bin)?,
                    U32 => self.exec_mod::<u32>(bin)?,
                    I32 => self.exec_mod::<i32>(bin)?,
                    U64 => self.exec_mod::<u64>(bin)?,
                    I64 => self.exec_mod::<i64>(bin)?,
                    Uw => self.exec_mod::<UWord>(bin)?,
                    Iw => self.exec_mod::<IWord>(bin)?,
                    F32 => self.exec_mod::<f32>(bin)?,
                    F64 => self.exec_mod::<f64>(bin)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Shl(x, y, ot) => {
                match ot {
                    U8 => self.exec_shl::<u8>(x, y)?,
                    I8 => self.exec_shl::<i8>(x, y)?,
                    U16 => self.exec_shl::<u16>(x, y)?,
                    I16 => self.exec_shl::<i16>(x, y)?,
                    U32 => self.exec_shl::<u32>(x, y)?,
                    I32 => self.exec_shl::<i32>(x, y)?,
                    U64 => self.exec_shl::<u64>(x, y)?,
                    I64 => self.exec_shl::<i64>(x, y)?,
                    Uw => self.exec_shl::<UWord>(x, y)?,
                    Iw => self.exec_shl::<IWord>(x, y)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Shr(x, y, ot) => {
                match ot {
                    U8 => self.exec_shr::<u8>(x, y)?,
                    I8 => self.exec_shr::<i8>(x, y)?,
                    U16 => self.exec_shr::<u16>(x, y)?,
                    I16 => self.exec_shr::<i16>(x, y)?,
                    U32 => self.exec_shr::<u32>(x, y)?,
                    I32 => self.exec_shr::<i32>(x, y)?,
                    U64 => self.exec_shr::<u64>(x, y)?,
                    I64 => self.exec_shr::<i64>(x, y)?,
                    Uw => self.exec_shr::<UWord>(x, y)?,
                    Iw => self.exec_shr::<IWord>(x, y)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            And(bin, ot) => {
                match ot {
                    U8 => self.exec_and::<u8>(bin)?,
                    I8 => self.exec_and::<i8>(bin)?,
                    U16 => self.exec_and::<u16>(bin)?,
                    I16 => self.exec_and::<i16>(bin)?,
                    U32 => self.exec_and::<u32>(bin)?,
                    I32 => self.exec_and::<i32>(bin)?,
                    U64 => self.exec_and::<u64>(bin)?,
                    I64 => self.exec_and::<i64>(bin)?,
                    Uw => self.exec_and::<UWord>(bin)?,
                    Iw => self.exec_and::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Or(bin, ot) => {
                match ot {
                    U8 => self.exec_or::<u8>(bin)?,
                    I8 => self.exec_or::<i8>(bin)?,
                    U16 => self.exec_or::<u16>(bin)?,
                    I16 => self.exec_or::<i16>(bin)?,
                    U32 => self.exec_or::<u32>(bin)?,
                    I32 => self.exec_or::<i32>(bin)?,
                    U64 => self.exec_or::<u64>(bin)?,
                    I64 => self.exec_or::<i64>(bin)?,
                    Uw => self.exec_or::<UWord>(bin)?,
                    Iw => self.exec_or::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Xor(bin, ot) => {
                match ot {
                    U8 => self.exec_xor::<u8>(bin)?,
                    I8 => self.exec_xor::<i8>(bin)?,
                    U16 => self.exec_xor::<u16>(bin)?,
                    I16 => self.exec_xor::<i16>(bin)?,
                    U32 => self.exec_xor::<u32>(bin)?,
                    I32 => self.exec_xor::<i32>(bin)?,
                    U64 => self.exec_xor::<u64>(bin)?,
                    I64 => self.exec_xor::<i64>(bin)?,
                    Uw => self.exec_xor::<UWord>(bin)?,
                    Iw => self.exec_xor::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Not(un, ot) => {
                match ot {
                    U8 => self.exec_not::<u8>(un)?,
                    I8 => self.exec_not::<i8>(un)?,
                    U16 => self.exec_not::<u16>(un)?,
                    I16 => self.exec_not::<i16>(un)?,
                    U32 => self.exec_not::<u32>(un)?,
                    I32 => self.exec_not::<i32>(un)?,
                    U64 => self.exec_not::<u64>(un)?,
                    I64 => self.exec_not::<i64>(un)?,
                    Uw => self.exec_not::<UWord>(un)?,
                    Iw => self.exec_not::<IWord>(un)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                }

                Ok(ExecutionSuccess::Ok)
            }
            Neg(un, ot) => {
                match ot {
                    U8 => self.exec_neg::<u8>(un)?,
                    I8 => self.exec_neg::<i8>(un)?,
                    U16 => self.exec_neg::<u16>(un)?,
                    I16 => self.exec_neg::<i16>(un)?,
                    U32 => self.exec_neg::<u32>(un)?,
                    I32 => self.exec_neg::<i32>(un)?,
                    U64 => self.exec_neg::<u64>(un)?,
                    I64 => self.exec_neg::<i64>(un)?,
                    Uw => self.exec_neg::<UWord>(un)?,
                    Iw => self.exec_neg::<IWord>(un)?,
                    F32 => self.exec_neg::<f32>(un)?,
                    F64 => self.exec_neg::<f64>(un)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Inc(un, ot) => {
                match ot {
                    U8 => self.exec_inc::<u8>(un)?,
                    I8 => self.exec_inc::<i8>(un)?,
                    U16 => self.exec_inc::<u16>(un)?,
                    I16 => self.exec_inc::<i16>(un)?,
                    U32 => self.exec_inc::<u32>(un)?,
                    I32 => self.exec_inc::<i32>(un)?,
                    U64 => self.exec_inc::<u64>(un)?,
                    I64 => self.exec_inc::<i64>(un)?,
                    Uw => self.exec_inc::<UWord>(un)?,
                    Iw => self.exec_inc::<IWord>(un)?,
                    F32 => self.exec_inc::<f32>(un)?,
                    F64 => self.exec_inc::<f64>(un)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Dec(un, ot) => {
                match ot {
                    U8 => self.exec_dec::<u8>(un)?,
                    I8 => self.exec_dec::<i8>(un)?,
                    U16 => self.exec_dec::<u16>(un)?,
                    I16 => self.exec_dec::<i16>(un)?,
                    U32 => self.exec_dec::<u32>(un)?,
                    I32 => self.exec_dec::<i32>(un)?,
                    U64 => self.exec_dec::<u64>(un)?,
                    I64 => self.exec_dec::<i64>(un)?,
                    Uw => self.exec_dec::<UWord>(un)?,
                    Iw => self.exec_dec::<IWord>(un)?,
                    F32 => self.exec_dec::<f32>(un)?,
                    F64 => self.exec_dec::<f64>(un)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Go(x) => {
                self.program_counter = self.get_val(x)?;
                return Ok(ExecutionSuccess::Ok);
            }
            Ift(un, ot) => {
                let res = match ot {
                    U8 => self.get_un::<u8>(un)? != 0,
                    I8 => self.get_un::<i8>(un)? != 0,
                    U16 => self.get_un::<u16>(un)? != 0,
                    I16 => self.get_un::<i16>(un)? != 0,
                    U32 => self.get_un::<u32>(un)? != 0,
                    I32 => self.get_un::<i32>(un)? != 0,
                    U64 => self.get_un::<u64>(un)? != 0,
                    I64 => self.get_un::<i64>(un)? != 0,
                    Uw => self.get_un::<UWord>(un)? != 0,
                    Iw => self.get_un::<IWord>(un)? != 0,
                    F32 => self.get_un::<f32>(un)? != 0.0,
                    F64 => self.get_un::<f64>(un)? != 0.0,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Iff(un, ot) => {
                let res = match ot {
                    U8 => self.get_un::<u8>(un)? == 0,
                    I8 => self.get_un::<i8>(un)? == 0,
                    U16 => self.get_un::<u16>(un)? == 0,
                    I16 => self.get_un::<i16>(un)? == 0,
                    U32 => self.get_un::<u32>(un)? == 0,
                    I32 => self.get_un::<i32>(un)? == 0,
                    U64 => self.get_un::<u64>(un)? == 0,
                    I64 => self.get_un::<i64>(un)? == 0,
                    Uw => self.get_un::<UWord>(un)? == 0,
                    Iw => self.get_un::<IWord>(un)? == 0,
                    F32 => self.get_un::<f32>(un)? == 0.0,
                    F64 => self.get_un::<f64>(un)? == 0.0,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ife(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ife::<u8>(bin)?,
                    I8 => self.exec_ife::<i8>(bin)?,
                    U16 => self.exec_ife::<u16>(bin)?,
                    I16 => self.exec_ife::<i16>(bin)?,
                    U32 => self.exec_ife::<u32>(bin)?,
                    I32 => self.exec_ife::<i32>(bin)?,
                    U64 => self.exec_ife::<u64>(bin)?,
                    I64 => self.exec_ife::<i64>(bin)?,
                    Uw => self.exec_ife::<UWord>(bin)?,
                    Iw => self.exec_ife::<IWord>(bin)?,
                    F32 => self.exec_ife::<f32>(bin)?,
                    F64 => self.exec_ife::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ifl(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ifl::<u8>(bin)?,
                    I8 => self.exec_ifl::<i8>(bin)?,
                    U16 => self.exec_ifl::<u16>(bin)?,
                    I16 => self.exec_ifl::<i16>(bin)?,
                    U32 => self.exec_ifl::<u32>(bin)?,
                    I32 => self.exec_ifl::<i32>(bin)?,
                    U64 => self.exec_ifl::<u64>(bin)?,
                    I64 => self.exec_ifl::<i64>(bin)?,
                    Uw => self.exec_ifl::<UWord>(bin)?,
                    Iw => self.exec_ifl::<IWord>(bin)?,
                    F32 => self.exec_ifl::<f32>(bin)?,
                    F64 => self.exec_ifl::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ifg(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ifg::<u8>(bin)?,
                    I8 => self.exec_ifg::<i8>(bin)?,
                    U16 => self.exec_ifg::<u16>(bin)?,
                    I16 => self.exec_ifg::<i16>(bin)?,
                    U32 => self.exec_ifg::<u32>(bin)?,
                    I32 => self.exec_ifg::<i32>(bin)?,
                    U64 => self.exec_ifg::<u64>(bin)?,
                    I64 => self.exec_ifg::<i64>(bin)?,
                    Uw => self.exec_ifg::<UWord>(bin)?,
                    Iw => self.exec_ifg::<IWord>(bin)?,
                    F32 => self.exec_ifg::<f32>(bin)?,
                    F64 => self.exec_ifg::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ine(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ine::<u8>(bin)?,
                    I8 => self.exec_ine::<i8>(bin)?,
                    U16 => self.exec_ine::<u16>(bin)?,
                    I16 => self.exec_ine::<i16>(bin)?,
                    U32 => self.exec_ine::<u32>(bin)?,
                    I32 => self.exec_ine::<i32>(bin)?,
                    U64 => self.exec_ine::<u64>(bin)?,
                    I64 => self.exec_ine::<i64>(bin)?,
                    Uw => self.exec_ine::<UWord>(bin)?,
                    Iw => self.exec_ine::<IWord>(bin)?,
                    F32 => self.exec_ine::<f32>(bin)?,
                    F64 => self.exec_ine::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Inl(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_inl::<u8>(bin)?,
                    I8 => self.exec_inl::<i8>(bin)?,
                    U16 => self.exec_inl::<u16>(bin)?,
                    I16 => self.exec_inl::<i16>(bin)?,
                    U32 => self.exec_inl::<u32>(bin)?,
                    I32 => self.exec_inl::<i32>(bin)?,
                    U64 => self.exec_inl::<u64>(bin)?,
                    I64 => self.exec_inl::<i64>(bin)?,
                    Uw => self.exec_inl::<UWord>(bin)?,
                    Iw => self.exec_inl::<IWord>(bin)?,
                    F32 => self.exec_inl::<f32>(bin)?,
                    F64 => self.exec_inl::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ing(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ing::<u8>(bin)?,
                    I8 => self.exec_ing::<i8>(bin)?,
                    U16 => self.exec_ing::<u16>(bin)?,
                    I16 => self.exec_ing::<i16>(bin)?,
                    U32 => self.exec_ing::<u32>(bin)?,
                    I32 => self.exec_ing::<i32>(bin)?,
                    U64 => self.exec_ing::<u64>(bin)?,
                    I64 => self.exec_ing::<i64>(bin)?,
                    Uw => self.exec_ing::<UWord>(bin)?,
                    Iw => self.exec_ing::<IWord>(bin)?,
                    F32 => self.exec_ing::<f32>(bin)?,
                    F64 => self.exec_ing::<f64>(bin)?,
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ifa(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ifa::<u8>(bin)?,
                    I8 => self.exec_ifa::<i8>(bin)?,
                    U16 => self.exec_ifa::<u16>(bin)?,
                    I16 => self.exec_ifa::<i16>(bin)?,
                    U32 => self.exec_ifa::<u32>(bin)?,
                    I32 => self.exec_ifa::<i32>(bin)?,
                    U64 => self.exec_ifa::<u64>(bin)?,
                    I64 => self.exec_ifa::<i64>(bin)?,
                    Uw => self.exec_ifa::<UWord>(bin)?,
                    Iw => self.exec_ifa::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ifo(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ifo::<u8>(bin)?,
                    I8 => self.exec_ifo::<i8>(bin)?,
                    U16 => self.exec_ifo::<u16>(bin)?,
                    I16 => self.exec_ifo::<i16>(bin)?,
                    U32 => self.exec_ifo::<u32>(bin)?,
                    I32 => self.exec_ifo::<i32>(bin)?,
                    U64 => self.exec_ifo::<u64>(bin)?,
                    I64 => self.exec_ifo::<i64>(bin)?,
                    Uw => self.exec_ifo::<UWord>(bin)?,
                    Iw => self.exec_ifo::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ifx(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ifx::<u8>(bin)?,
                    I8 => self.exec_ifx::<i8>(bin)?,
                    U16 => self.exec_ifx::<u16>(bin)?,
                    I16 => self.exec_ifx::<i16>(bin)?,
                    U32 => self.exec_ifx::<u32>(bin)?,
                    I32 => self.exec_ifx::<i32>(bin)?,
                    U64 => self.exec_ifx::<u64>(bin)?,
                    I64 => self.exec_ifx::<i64>(bin)?,
                    Uw => self.exec_ifx::<UWord>(bin)?,
                    Iw => self.exec_ifx::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ina(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ina::<u8>(bin)?,
                    I8 => self.exec_ina::<i8>(bin)?,
                    U16 => self.exec_ina::<u16>(bin)?,
                    I16 => self.exec_ina::<i16>(bin)?,
                    U32 => self.exec_ina::<u32>(bin)?,
                    I32 => self.exec_ina::<i32>(bin)?,
                    U64 => self.exec_ina::<u64>(bin)?,
                    I64 => self.exec_ina::<i64>(bin)?,
                    Uw => self.exec_ina::<UWord>(bin)?,
                    Iw => self.exec_ina::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Ino(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_ino::<u8>(bin)?,
                    I8 => self.exec_ino::<i8>(bin)?,
                    U16 => self.exec_ino::<u16>(bin)?,
                    I16 => self.exec_ino::<i16>(bin)?,
                    U32 => self.exec_ino::<u32>(bin)?,
                    I32 => self.exec_ino::<i32>(bin)?,
                    U64 => self.exec_ino::<u64>(bin)?,
                    I64 => self.exec_ino::<i64>(bin)?,
                    Uw => self.exec_ino::<UWord>(bin)?,
                    Iw => self.exec_ino::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Inx(bin, ot) => {
                let res = match ot {
                    U8 => self.exec_inx::<u8>(bin)?,
                    I8 => self.exec_inx::<i8>(bin)?,
                    U16 => self.exec_inx::<u16>(bin)?,
                    I16 => self.exec_inx::<i16>(bin)?,
                    U32 => self.exec_inx::<u32>(bin)?,
                    I32 => self.exec_inx::<i32>(bin)?,
                    U64 => self.exec_inx::<u64>(bin)?,
                    I64 => self.exec_inx::<i64>(bin)?,
                    Uw => self.exec_inx::<UWord>(bin)?,
                    Iw => self.exec_inx::<IWord>(bin)?,
                    F32 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                    F64 => return Err(ExecutionError::IncorrectOperation(*self.current_op()?)),
                };

                if res {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            App(x) => {
                self.app(self.get_val(x)?)?;
                Ok(ExecutionSuccess::Ok)
            }
            Par(un, ot) => {
                match ot {
                    U8 => self.exec_par::<u8>(un)?,
                    I8 => self.exec_par::<i8>(un)?,
                    U16 => self.exec_par::<u16>(un)?,
                    I16 => self.exec_par::<i16>(un)?,
                    U32 => self.exec_par::<u32>(un)?,
                    I32 => self.exec_par::<i32>(un)?,
                    U64 => self.exec_par::<u64>(un)?,
                    I64 => self.exec_par::<i64>(un)?,
                    Uw => self.exec_par::<UWord>(un)?,
                    Iw => self.exec_par::<IWord>(un)?,
                    F32 => self.exec_par::<f32>(un)?,
                    F64 => self.exec_par::<f64>(un)?,
                }

                Ok(ExecutionSuccess::Ok)
            }
            Clf(x) => {
                self.clf(self.get_val(x)?)?;
                return Ok(ExecutionSuccess::Ok);
            }
            Ret(un, ot) => {
                if un.x() != Operand::Emp {
                    match ot {
                        U8 => self.set_ret::<u8>(un)?,
                        I8 => self.set_ret::<i8>(un)?,
                        U16 => self.set_ret::<u16>(un)?,
                        I16 => self.set_ret::<i16>(un)?,
                        U32 => self.set_ret::<u32>(un)?,
                        I32 => self.set_ret::<i32>(un)?,
                        U64 => self.set_ret::<u64>(un)?,
                        I64 => self.set_ret::<i64>(un)?,
                        Uw => self.set_ret::<UWord>(un)?,
                        Iw => self.set_ret::<IWord>(un)?,
                        F32 => self.set_ret::<f32>(un)?,
                        F64 => self.set_ret::<f64>(un)?,
                    }
                }

                self.ret()?;
                return Ok(ExecutionSuccess::Ok);
            }
            In(bin) => {
                let val = self.files.read()?;
                let (left, right) = self.read_bin_operands(bin)?;

                if right != Operand::Emp {
                    self.set_val::<u8>(right, if val.is_some() { 1 } else { 0 })?;
                }

                self.set_val(left, val.unwrap_or(0))?;
                Ok(ExecutionSuccess::Ok)
            }
            Out(un) => {
                let val = self.get_val(self.read_un_operand(un)?)?;
                self.files.write(val)?;
                Ok(ExecutionSuccess::Ok)
            }
            Fls => {
                self.files.flush()?;
                Ok(ExecutionSuccess::Ok)
            }
            Sfd(x) => {
                self.files.set_current(self.get_val(x)?)?;
                Ok(ExecutionSuccess::Ok)
            }
            Gfd(x) => {
                self.set_val(x, self.files.current()?)?;
                Ok(ExecutionSuccess::Ok)
            }
            Zer(x, y) => {
                let dest = self.get_val(x)?;
                let size = self.get_val(y)?;
                self.memory.set_zeros(dest, size)?;
                Ok(ExecutionSuccess::Ok)
            }
            Cmp(x, y, z) => {
                let a = self.get_val(x)?;
                let b = self.get_val(y)?;
                let size = self.get_val(z)?;

                if self.memory.compare(a, b, size)? {
                    Ok(ExecutionSuccess::Ok)
                } else {
                    self.pass_condition()?;
                    return Ok(ExecutionSuccess::Ok);
                }
            }
            Cpy(x, y, z) => {
                let dest = self.get_val(x)?;
                let src = self.get_val(y)?;
                let size = self.get_val(z)?;
                self.memory.copy(dest, src, size)?;
                Ok(ExecutionSuccess::Ok)
            }
        };

        if res.is_ok() {
            self.program_counter = self.program_counter.wrapping_add(1);
        }

        res
    }
}
