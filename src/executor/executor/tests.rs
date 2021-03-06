use super::*;

#[test]
fn executor_set_get_val() {
    let functions = [Function {
        frame_size: 8,
        program: &[Op::Nop],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();
    exe.call(0, 0).unwrap();

    assert_eq!(exe.set_val(Operand::Loc(0), 8), Ok(()));
    assert_eq!(exe.get_val::<usize>(Operand::Loc(0)), Ok(8));

    assert_eq!(exe.set_val(Operand::Glb(0), 8), Ok(()));
    assert_eq!(exe.get_val::<usize>(Operand::Glb(0)), Ok(8));

    let null_deref_err = ExecutionError::NullPointerDereference;
    assert_eq!(exe.set_val(Operand::Ind(0), 8), Err(null_deref_err));
    assert_eq!(exe.get_val::<usize>(Operand::Ind(0)), Err(null_deref_err));

    assert_eq!(exe.set_val(Operand::Ret(0), 3), Ok(()));
    assert_eq!(exe.get_val::<usize>(Operand::Ret(0)), Ok(3));

    assert_eq!(
        exe.set_val(Operand::Val(7), 0),
        Err(ExecutionError::IncorrectOperation(Op::Nop)),
    );
    assert_eq!(exe.get_val::<usize>(Operand::Val(8)), Ok(8));

    assert_eq!(
        exe.set_val(Operand::Ref(0), 0),
        Err(ExecutionError::IncorrectOperation(Op::Nop)),
    );
    assert_eq!(exe.get_val::<usize>(Operand::Ref(0)), Ok(8));

    assert_eq!(
        exe.set_val(Operand::Emp, 0),
        Err(ExecutionError::IncorrectOperation(Op::Nop)),
    );
    assert_eq!(
        exe.get_val::<usize>(Operand::Emp),
        Err(ExecutionError::IncorrectOperation(Op::Nop)),
    );
}

#[test]
fn executor_set() {
    let fb = f32::to_le_bytes(0.123);
    let float = UWord::from_slice(fb.as_ref());

    let functions = [Function {
        frame_size: 4,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(12)), OpType::I32),
            Op::Set(BinOp::new(Operand::Val(0), Operand::Val(12)), OpType::I32),
            Op::Set(BinOp::new(Operand::Emp, Operand::Val(12)), OpType::I32),
            Op::Set(BinOp::new(Operand::Loc(1), Operand::Val(32)), OpType::I8),
            Op::Set(
                BinOp::new(Operand::Loc(0), Operand::Val(float)),
                OpType::F32,
            ),
            Op::Set(
                BinOp::new(Operand::Glb(0), Operand::Val(float)),
                OpType::F32,
            ),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(12));

    assert_eq!(
        exe.execute(),
        Executed::Err(ExecutionError::IncorrectOperation(Op::Set(
            BinOp::new(Operand::Val(0), Operand::Val(12)),
            OpType::I32
        )))
    );
    exe.program_counter += 1; // Move manually after incorrect operation

    assert_eq!(
        exe.execute(),
        Executed::Err(ExecutionError::IncorrectOperation(Op::Set(
            BinOp::new(Operand::Emp, Operand::Val(12)),
            OpType::I32
        )))
    );
    exe.program_counter += 1; // Move manually after incorrect operation

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i8>(Operand::Loc(1)), Ok(32));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<f32>(Operand::Loc(0)), Ok(0.123));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<f32>(Operand::Loc(0)), Ok(0.123));
}

#[test]
fn executor_cnv() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::I64),
            Op::Cnv(Operand::Loc(0), Operand::Loc(0), OpType::I64, OpType::U8),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(0)), Ok(2));
}

#[test]
fn executor_shl() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::U32),
            Op::Shl(Operand::Loc(0), Operand::Val(1), OpType::U32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(4));
}

#[test]
fn executor_shr() {
    let functions = [Function {
        frame_size: 9,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(8), Operand::Val(1)), OpType::U8),
            Op::Shr(Operand::Loc(0), Operand::Loc(8), OpType::U32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(1));
}

#[test]
fn executor_add() {
    let functions = [Function {
        frame_size: 4,
        program: &[
            Op::Add(BinOp::new(Operand::Loc(0), Operand::Val(12)), OpType::I32),
            Op::Add(
                BinOp::new(Operand::Loc(0), Operand::Val(u32::MAX as UWord)),
                OpType::I32,
            ),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::I32),
            Op::Add(
                BinOp::new(Operand::Loc(0), Operand::Val(i32::MAX as UWord)),
                OpType::I32,
            ),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(12));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(11));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(1));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(i32::MIN));
}

#[test]
fn executor_mul() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(8)), OpType::I32),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(5)), OpType::I32),
            Op::Mul(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::I32),
            Op::Mul(BinOp::new(Operand::Loc(4), Operand::Val(2)), OpType::I32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(16));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(4)), Ok(10));
}

#[test]
fn executor_div() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(8)), OpType::I32),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(5)), OpType::I32),
            Op::Div(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::I32),
            Op::Div(BinOp::new(Operand::Loc(4), Operand::Val(2)), OpType::I32),
            Op::Div(BinOp::new(Operand::Loc(0), Operand::Val(0)), OpType::I32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(4));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<i32>(Operand::Loc(4)), Ok(2));
    assert_eq!(exe.execute(), Executed::Err(ExecutionError::DivisionByZero));
}

#[test]
fn executor_go() {
    let functions = [Function {
        frame_size: 4,
        program: &[
            Op::Inc(UnOp::new(Operand::Loc(0)), OpType::U32),
            Op::Go(Operand::Val(0)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(1));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(2));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(3));
}

#[test]
fn executor_ift() {
    let functions = [Function {
        frame_size: 1,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::U8),
            Op::Ift(UnOp::new(Operand::Loc(0)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::U8),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(0)), Ok(1));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(0)), Ok(2));
}

#[test]
fn executor_iff() {
    let functions = [Function {
        frame_size: 1,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::U8),
            Op::Iff(UnOp::new(Operand::Loc(0)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(2)), OpType::U8),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(0)), Ok(1));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(0)), Ok(1));
    assert_eq!(exe.execute(), Executed::Err(ExecutionError::EndOfProgram));
}

#[test]
fn executor_ife() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(32)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(32)), OpType::U32),
            Op::Ife(BinOp::new(Operand::Loc(0), Operand::Loc(4)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::U32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(32));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(4)), Ok(32));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(1));
}

#[test]
fn executor_ifa() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(32)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(2)), OpType::U32),
            Op::Ifa(BinOp::new(Operand::Loc(0), Operand::Loc(4)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::U32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(32));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(4)), Ok(2));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Err(ExecutionError::EndOfProgram));
}

#[test]
fn executor_ina() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(32)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(2)), OpType::U32),
            Op::Ina(BinOp::new(Operand::Loc(0), Operand::Loc(4)), OpType::U32),
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(1)), OpType::U32),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(32));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(4)), Ok(2));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(1));
}

#[test]
fn executor_call_fn() {
    let functions = [
        Function {
            frame_size: 4,
            program: &[
                Op::App(Operand::Val(1)),
                Op::Par(UnOp::new(Operand::Val(2)), OpType::I32),
                Op::Clf(Operand::Val(0)),
                Op::Ret(UnOp::new(Operand::Emp), OpType::U8),
            ],
        },
        Function {
            frame_size: 8,
            program: &[
                Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(3)), OpType::I32),
                Op::Add(BinOp::new(Operand::Ret(0), Operand::Loc(0)), OpType::I32),
                Op::Add(BinOp::new(Operand::Ret(0), Operand::Loc(4)), OpType::I32),
                Op::Ret(UnOp::new(Operand::Emp), OpType::U8),
            ],
        },
    ];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.call_stack.len(), 2);

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.call_stack.len(), 1);

    assert_eq!(exe.get_val::<i32>(Operand::Loc(0)), Ok(5));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert!(exe.call_stack.is_empty());
}

#[test]
fn executor_glb() {
    let functions = [
        Function {
            frame_size: 0,
            program: &[
                Op::Set(BinOp::new(Operand::Glb(2), Operand::Val(12)), OpType::U16),
                Op::App(Operand::Val(1)),
                Op::Par(UnOp::new(Operand::Val(6)), OpType::U16),
                Op::Clf(Operand::Val(0)),
                Op::Ret(UnOp::new(Operand::Emp), OpType::U8),
            ],
        },
        Function {
            frame_size: 2,
            program: &[
                Op::Inc(UnOp::new(Operand::Loc(0)), OpType::U16),
                Op::Set(BinOp::new(Operand::Glb(0), Operand::Loc(0)), OpType::U16),
                Op::Ret(UnOp::new(Operand::Emp), OpType::U8),
            ],
        },
    ];

    let mut exe = Executor::new(&functions);
    exe.memory.stack.expand(8).unwrap();
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.call_stack.len(), 2);
    assert_eq!(exe.get_val::<u16>(Operand::Glb(2)), Ok(12));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.call_stack.len(), 1);
    assert_eq!(exe.get_val::<u16>(Operand::Glb(0)), Ok(7));

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert!(exe.call_stack.is_empty());
    assert_eq!(exe.memory.stack.len(), 8);
}

#[test]
fn executor_gcd() {
    let functions = [
        Function {
            frame_size: 12,
            program: &[
                // u32 result
                // u32 x
                // u32 y
                // set x 234
                Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(234)), OpType::U32),
                // set y 533
                Op::Set(BinOp::new(Operand::Loc(8), Operand::Val(533)), OpType::U32),
                // app gcd
                Op::App(Operand::Val(1)),
                // par x
                Op::Par(UnOp::new(Operand::Loc(4)), OpType::U32),
                // par y
                Op::Par(UnOp::new(Operand::Loc(8)), OpType::U32),
                // clf result
                Op::Clf(Operand::Val(0)),
                // end
                Op::End(Operand::Val(0)),
            ],
        },
        Function {
            // fn gcd
            frame_size: 12,
            program: &[
                // u32 a
                // u32 b
                // u32 c
                // loop:
                // set c a
                Op::Set(BinOp::new(Operand::Loc(8), Operand::Loc(0)), OpType::U32),
                // mod c b
                Op::Mod(BinOp::new(Operand::Loc(8), Operand::Loc(4)), OpType::U32),
                // set a b
                Op::Set(BinOp::new(Operand::Loc(0), Operand::Loc(4)), OpType::U32),
                // set b c
                Op::Set(BinOp::new(Operand::Loc(4), Operand::Loc(8)), OpType::U32),
                // ift b
                Op::Ift(UnOp::new(Operand::Loc(4)), OpType::U32),
                // go loop
                Op::Go(Operand::Val(0)),
                // ret a
                Op::Ret(UnOp::new(Operand::Loc(0)), OpType::U32),
            ],
        },
    ];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    let mut executed = Executed::Ok(ExecutionSuccess::Ok);
    while let Executed::Ok(ExecutionSuccess::Ok) = executed {
        executed = exe.execute();
    }

    assert_eq!(executed, Executed::Ok(ExecutionSuccess::End(0)));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(13));
}

#[test]
fn executor_hello() {
    let functions = [Function {
        frame_size: 6 + std::mem::size_of::<UWord>() as UWord,
        program: &[
            // u8[6] hello // "Hello!"
            Op::Set(
                BinOp::new(Operand::Loc(0), Operand::Val('H' as UWord)),
                OpType::U8,
            ),
            Op::Set(
                BinOp::new(Operand::Loc(1), Operand::Val('e' as UWord)),
                OpType::U8,
            ),
            Op::Set(
                BinOp::new(Operand::Loc(2), Operand::Val('l' as UWord)),
                OpType::U8,
            ),
            Op::Set(
                BinOp::new(Operand::Loc(3), Operand::Val('l' as UWord)),
                OpType::U8,
            ),
            Op::Set(
                BinOp::new(Operand::Loc(4), Operand::Val('o' as UWord)),
                OpType::U8,
            ),
            Op::Set(
                BinOp::new(Operand::Loc(5), Operand::Val('!' as UWord)),
                OpType::U8,
            ),
            //
            // uw i
            // set i 0
            Op::Set(BinOp::new(Operand::Loc(6), Operand::Val(0)), OpType::Uw),
            // loop:
            // out hello{i}
            Op::Out(UnOp::new(Operand::Loc(0)).with_first(Operand::Loc(6))),
            // inc i
            Op::Inc(UnOp::new(Operand::Loc(6)), OpType::Uw),
            // ifl i 6
            Op::Ifl(BinOp::new(Operand::Loc(6), Operand::Val(6)), OpType::Uw),
            // go loop
            Op::Go(Operand::Val(7)),
            // end
            Op::End(Operand::Val(0)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    let file: Vec<u8> = Vec::new();

    assert_eq!(exe.files.open(file), Ok(0));
    exe.files.set_current(0).unwrap();
    exe.call(0, 0).unwrap();

    let mut executed = Executed::Ok(ExecutionSuccess::Ok);
    while let Executed::Ok(ExecutionSuccess::Ok) = executed {
        executed = exe.execute();
    }

    assert_eq!(executed, Executed::Ok(ExecutionSuccess::End(0)));
    let file = exe.files.close(0).unwrap();
    let slice = file.as_any().downcast_ref::<Vec<u8>>().unwrap().as_slice();

    let hello = String::from_utf8_lossy(slice);
    assert_eq!(hello, "Hello!");
}

#[test]
fn executor_mul_from_in() {
    use std::collections::vec_deque::VecDeque;

    let functions = [Function {
        frame_size: 3 + std::mem::size_of::<UWord>() as UWord,
        program: &[
            // u8 a
            // u8 b
            // u8 eof
            // uw res
            // in a
            Op::In(BinOp::new(Operand::Loc(0), Operand::Emp)),
            // in b eof
            Op::In(BinOp::new(Operand::Loc(1), Operand::Loc(2))),
            // set res a
            Op::Set(BinOp::new(Operand::Loc(3), Operand::Loc(0)), OpType::U8),
            // mul res b
            Op::Mul(BinOp::new(Operand::Loc(3), Operand::Loc(1)), OpType::U8),
            // end
            Op::End(Operand::Loc(3)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    let mut file: VecDeque<u8> = VecDeque::new();
    file.push_back(3);
    file.push_back(4);

    assert_eq!(exe.files.open(file), Ok(0));
    exe.files.set_current(0).unwrap();
    exe.call(0, 0).unwrap();

    let mut executed = Executed::Ok(ExecutionSuccess::Ok);
    while let Executed::Ok(ExecutionSuccess::Ok) = executed {
        executed = exe.execute();
    }

    assert_eq!(executed, Executed::Ok(ExecutionSuccess::End(3 * 4)));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(2)), Ok(1));
}

#[test]
fn executor_zer() {
    let functions = [Function {
        frame_size: 16,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(0xFF)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(8), Operand::Val(0xFF)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(15), Operand::Val(0xFF)), OpType::U8),
            Op::Zer(Operand::Val(0), Operand::Val(16)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.get_val::<u64>(Operand::Loc(0)), Ok(0));
    assert_eq!(exe.get_val::<u64>(Operand::Loc(8)), Ok(0));
}

#[test]
fn executor_cmp() {
    let functions = [Function {
        frame_size: 5,
        program: &[
            Op::Set(BinOp::new(Operand::Loc(0), Operand::Val(0xFF)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(1), Operand::Val(0xFF)), OpType::U8),
            Op::Set(BinOp::new(Operand::Loc(2), Operand::Val(0x02)), OpType::U8),
            Op::Cmp(Operand::Val(0), Operand::Val(1), Operand::Val(1)),
            Op::Set(BinOp::new(Operand::Loc(3), Operand::Val(1)), OpType::U8),
            Op::Cmp(Operand::Val(0), Operand::Val(2), Operand::Val(1)),
            Op::Set(BinOp::new(Operand::Loc(4), Operand::Val(1)), OpType::U8),
            Op::End(Operand::Val(0)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    let mut executed = Executed::Ok(ExecutionSuccess::Ok);
    while let Executed::Ok(ExecutionSuccess::Ok) = executed {
        executed = exe.execute();
    }

    assert_eq!(executed, Executed::Ok(ExecutionSuccess::End(0)));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(3)), Ok(1));
    assert_eq!(exe.get_val::<u8>(Operand::Loc(4)), Ok(0));
}

#[test]
fn executor_cpy() {
    let functions = [Function {
        frame_size: 8,
        program: &[
            Op::Set(
                BinOp::new(Operand::Loc(0), Operand::Val(0x10EF)),
                OpType::U32,
            ),
            Op::Cpy(Operand::Val(4), Operand::Val(0), Operand::Val(4)),
        ],
    }];

    let mut exe = Executor::new(&functions);
    exe.call(0, 0).unwrap();

    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));
    assert_eq!(exe.execute(), Executed::Ok(ExecutionSuccess::Ok));

    assert_eq!(exe.get_val::<u32>(Operand::Loc(0)), Ok(0x10EF));
    assert_eq!(exe.get_val::<u32>(Operand::Loc(4)), Ok(0x10EF));
}
