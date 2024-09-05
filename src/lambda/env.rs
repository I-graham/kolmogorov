use super::*;
use std::rc::Rc;

type BuiltInFunc = Rc<dyn Fn(&[Thunk]) -> Option<NTerm>>;

#[derive(Clone)]
pub struct BuiltIn {
	pub n_args: usize,
	pub func: BuiltInFunc,
	pub ty: Rc<Type>,
}
