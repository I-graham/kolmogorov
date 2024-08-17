use super::*;

use rustc_hash::FxHashMap as HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SearchResult {
	#[default]
	Unknown,
	Inhabited,
	Uninhabited,
}

type Search = (Rc<Type>, usize);
type PathDict = HashMap<Search, SearchResult>;

pub struct Cache {
	paths: Vec<PathDict>,
	searches: Vec<Search>,
}

use SearchResult::*;
impl Cache {
	pub fn new() -> Self {
		Self {
			paths: vec![Default::default()],
			searches: vec![],
		}
	}

	pub fn intro_var(&mut self, _is_new: bool) {
		self.paths.push(Default::default());
	}

	pub fn elim_var(&mut self) {
		self.paths.pop();
	}

	pub fn prune(&self, targ: &Rc<Type>, size: usize) -> bool {
		let search = (targ.clone(), size);

		self.active().get(&search) == Some(&Uninhabited)
	}

	pub fn prune_arg(&self, targ: &Rc<Type>, l_ty: &Rc<Type>, size: usize) -> SearchResult {
		fn core(dict: &PathDict, targ: &Rc<Type>, l_ty: &Rc<Type>, size: usize) -> SearchResult {
			let last = l_ty == targ;

			if size == 0 && last {
				return Inhabited;
			}

			if size == 0 || last {
				return Uninhabited;
			}

			let Type::Fun(arg, ret) = &**l_ty else {
				unreachable!()
			};

			let mut res = Uninhabited;
			for n in 1..size {
				let search = (arg.clone(), n);
				let arg_res = *dict.get(&search).unwrap_or(&Unknown);

				if arg_res == Uninhabited {
					continue;
				}

				let rest = core(dict, targ, ret, size - n - 1);

				if arg_res == Unknown && matches!(rest, Unknown | Inhabited) {
					res = Unknown;
					continue;
				}

				if arg_res == Inhabited && rest == Inhabited {
					res = Inhabited;
					break;
				}
			}

			res
		}

		core(self.active(), targ, l_ty, size)
	}

	pub fn begin_search(&mut self, targ: &Rc<Type>, size: usize) {
		let search = (targ.clone(), size);

		self.searches.push(search.clone());

		self.active_mut().entry(search).or_insert(Unknown);
	}

	pub fn yield_term(&mut self, targ: &Rc<Type>, size: usize) {
		let search = (targ.clone(), size);

		self.active_mut().insert(search, Inhabited);
	}

	pub fn end_search(&mut self) {
		let search = self.searches.pop().unwrap();

		let result = self.active_mut().get_mut(&search).unwrap();

		if *result == Unknown {
			*result = Uninhabited;
		}
	}

	fn active(&self) -> &PathDict {
		self.paths.last().unwrap()
	}

	fn active_mut(&mut self) -> &mut PathDict {
		self.paths.last_mut().unwrap()
	}
}
