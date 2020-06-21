use common::{
    OpType,
    UWord,
};

use super::{
    WORD_SIZE,
    view::View,
    LayoutBuilder,
};

#[derive(Debug)]
pub struct Layout<'n, 't> {
    pub(super) fields: View<Field<'n, 't>>,
    pub(super) types: View<Ty<'t>>,
}

impl<'n, 't> Layout<'n, 't> {
    pub fn builder() -> LayoutBuilder<'n> { LayoutBuilder::new() }

    pub fn size(&self, layouts: &[Layout]) -> UWord {
        self.fields.iter().map(|f| f.ty.size(layouts)).sum()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Field<'n, 't> {
    pub(super) name: &'n str,
    pub(super) ty: Ty<'t>,
    pub(super) ptr: UWord,
}

#[derive(Copy, Clone, Debug)]
pub enum Ty<'t> {
    OpType(OpType),
    Layout(usize),
    Array(&'t Ty<'t>, UWord),
    Indirect(&'t Ty<'t>),
    Function,
}

impl<'t> Ty<'t> {
    pub fn len(&self) -> UWord {
        match self {
            Ty::Array(_, len) => *len,
            _ => 1,
        }
    }

    pub fn size(&self, layouts: &[Layout]) -> UWord {
        match self {
            Ty::OpType(op) => op.size(),
            Ty::Layout(lay_idx) => layouts[*lay_idx].size(layouts),
            Ty::Array(&ty, len) => ty.size(layouts) * len,
            Ty::Indirect(_) => WORD_SIZE,
            Ty::Function => WORD_SIZE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        let lay = {
            let mut builder = Layout::builder();
            builder.new_fn("f");
            builder.new_op_type("x", OpType::U32);
            builder.add_indirect();
            builder.new_op_type("y", OpType::I32);
            builder.add_array(12);
            builder.add_array(4);
            builder.new_layout("self", 0);
            builder.add_indirect();
            builder.build().unwrap()
        };

        assert!(matches!(lay.fields[0].ty, Ty::Function));
        assert!(matches!(lay.fields[1].ty, Ty::Indirect(Ty::OpType(OpType::U32))));
        assert!(matches!(lay.fields[2].ty, Ty::Array(Ty::Array(Ty::OpType(OpType::I32), 12), 4)));
        assert!(matches!(lay.fields[3].ty, Ty::Indirect(Ty::Layout(0))));

        assert_eq!(
            lay.size(&[]),
            WORD_SIZE        // f
                + WORD_SIZE  // x
                + 4 * 12 * 4 // y
                + WORD_SIZE, // self
        );
    }
}
