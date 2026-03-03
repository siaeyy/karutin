// I DONT KNOW WHAT THE FUCK IS GOING ON HERE
// AND I AM SURE YOU WONT BE ABLE TO EITHER.
//
// SO GET THE FUCK OUT HERE AND GO THE `karutin` CRATE INSTEAD!
//
// <3
// ~ siaeyy 

use std::{
	collections::HashMap,
	iter::Peekable,
	ops::{Deref, DerefMut},
};

use syn::{
	Arm, Attribute, Block, Error, Expr, ExprBlock, ExprBreak, ExprForLoop, ExprIf, ExprLoop,
	ExprMatch, ExprReturn, ExprWhile, ExprYield, FnArg, GenericArgument, GenericParam, Generics,
	Ident, Lifetime, LifetimeParam, Local, Macro, Pat, PatType, Stmt, Token, Type, Visibility,
	parenthesized,
	parse::{Parse, ParseStream, discouraged::Speculative},
	parse_macro_input, parse_quote, parse_quote_spanned, parse2,
	punctuated::Punctuated,
	spanned::Spanned,
	token::{Comma, Paren, RArrow, Semi, Unsafe},
	visit::{self, Visit},
	visit_mut::{self, VisitMut},
};

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote, quote_spanned};

const COMPLETED_STATE_ID: usize = 0;
const LIFETIME_STR: &str = "'__karutin_lifetime__";
const STATE_LOOP_LABEL_STR: &str = "'__karutin_state_loop__";
const LET_BINDING_IDENT_STR: &str = "__karutin_let_binding__";

macro_rules! format_stack_ident {
	($i:expr) => {
		format_ident!("__{}_karutin_stack__", $i)
	};
}

macro_rules! format_context_ident {
	($i:expr) => {
		format_ident!("__{}_karutin_ctx__", $i)
	};
}

macro_rules! format_generic_ident {
	($i:expr) => {
		format_ident!("T{}", $i)
	};
}

macro_rules! format_field_ident {
	($i:expr) => {
		format_ident!("f{}", $i)
	};
}

fn is_yield_stmt(stmt: &Stmt) -> bool {
	match stmt {
		Stmt::Expr(Expr::Yield(_), _) => true,
		_ => false,
	}
}

fn is_loop_stmt(stmt: &Stmt) -> bool {
	match stmt {
		Stmt::Expr(Expr::Loop(_), _) => true,
		_ => false,
	}
}

fn is_break_stmt(stmt: &Stmt) -> bool {
	match stmt {
		Stmt::Expr(Expr::Break(_), _) => true,
		_ => false,
	}
}

#[derive(Default)]
struct PotentialYieldCheck(bool);

impl<'a> Visit<'a> for PotentialYieldCheck {
	fn visit_expr_yield(&mut self, node: &'a syn::ExprYield) {
		self.0 = true;
		visit::visit_expr_yield(self, node);
	}
}

fn is_potential_yield_stmt(stmt: &Stmt) -> bool {
	let mut check = PotentialYieldCheck::default();
	check.visit_stmt(stmt);
	check.0
}

fn convert_yield(expr_yield: &mut ExprYield) -> ExprBreak {
	let expr = expr_yield
		.expr
		.take()
		.unwrap_or_else(|| Box::new(parse_quote!(())));

	let state_loop_label = Lifetime::new(STATE_LOOP_LABEL_STR, Span::call_site());
	let span = expr_yield.yield_token.span;

	syn::parse_quote_spanned! {span=>
		break #state_loop_label ({ ::karutin::KarutinState::Yielded( #expr ) })
	}
}

fn convert_return(expr_return: &mut ExprReturn) -> ExprBlock {
	let expr = expr_return
		.expr
		.take()
		.unwrap_or_else(|| Box::new(parse_quote!(())));

	let state_loop_label = Lifetime::new(STATE_LOOP_LABEL_STR, Span::call_site());
	let span = expr_return.return_token.span;

	syn::parse_quote_spanned! {span=>{
		self.states[#COMPLETED_STATE_ID] = 1;
		break #state_loop_label ({ ::karutin::KarutinState::Returned( #expr ) })
	}}
}

fn continue_state_stmt(state_id: usize) -> Stmt {
	syn::parse_quote! {
		self.states[#state_id] += 1;
	}
}

fn create_state_arm(state: usize, block: Block) -> Arm {
	create_arm(syn::parse_quote! { #state }, block)
}

fn create_arm(pat: Pat, block: Block) -> Arm {
	syn::parse_quote! {
		#pat => #block
	}
}

fn chunk_by_statefuls(stmts: Vec<Stmt>) -> Vec<Vec<Stmt>> {
	stmts
		.chunk_by(|s1: &Stmt, s2: &Stmt| {
			!is_yield_stmt(s1)
				&& !is_potential_yield_stmt(s2)
				&& !is_loop_stmt(s1)
				&& !is_loop_stmt(s2)
		})
		.map(|c| c.to_owned())
		.collect::<Vec<Vec<Stmt>>>()
}

#[rustfmt::skip]
fn attach_state_match_arms(
    state_id: usize,
    match_expr: &mut ExprMatch,
    blocks: Vec<Block>,
) {
	let block_count = blocks.len();
 
    for (i, mut block) in blocks.into_iter().enumerate() {
        let stmt_count = block.stmts.len();
        block.stmts.push(continue_state_stmt(state_id));

		if is_break_stmt(&block.stmts[stmt_count - 1]) {
	        block.stmts.swap(stmt_count, stmt_count - 1);
		} else if i != block_count - 1 {
			let state_loop_label = Lifetime::new(
				STATE_LOOP_LABEL_STR,
				Span::call_site(),
			);
			
			block.stmts.push(parse_quote!{
				continue #state_loop_label;
			});
		}

        let arm = create_state_arm(i, block);
        match_expr.arms.push(arm);
    }

    let fall_arm = create_arm(
        syn::parse_quote! { _ },
        syn::parse_quote! { { } }
    );

    match_expr.arms.push(fall_arm);
}

const MACRO_USAGE_ERR: &str = "\
	Macros can not be used in Karutin functions!\n\n\
	Macros are not expandable in procedure macros,\n\
	so when code lowering, Karutin does not know what is going on in them.\n\
	Because of this the state machine and stack management do not work.\n\
	This may be solved in the future.
";

#[derive(Default)]
struct MacroSpans {
	inner: Vec<Span>,
}

impl MacroSpans {
	pub fn into_inner(self) -> Vec<Span> {
		self.inner
	}
}

impl<'a> Visit<'a> for MacroSpans {
	fn visit_macro(&mut self, macro_: &'a Macro) {
		self.inner.push(macro_.span());
	}
}

fn check_blocks_macro_usage(karutin_fn: &KarutinFn) -> Option<Error> {
	let mut macro_usage = MacroSpans::default();
	macro_usage.visit_block(&karutin_fn.block);

	macro_usage
		.into_inner()
		.into_iter()
		.map(|span| Error::new(span, MACRO_USAGE_ERR))
		.reduce(|mut acc, error| {
			acc.combine(error);
			acc
		})
}

const LET_BINDING_MUTABILITY_ERR: &str = "\
	Locals can not be immutable!\n\n\
	Because of the way stack is managed, locals are always moveable/mutable.\n\
	To prevent this, Karutin can follow moving/mutability, which is hard and even impossible for same cases.\n\
	So for now, we want to you explicitly define locals mutable for you to know what they are.\n\
	This may be solved in the future.
";

const COMPLEX_PATTERN_ERR: &str = "\
	Karutin functions can not have complex patterns!\n\n\
	Code lowering for this complex patterns and stack management are hard to implement.\n\
	So for now, only simple patterns are available.\n\
	This may be solved in the future.
";

#[derive(PartialEq, Eq, Hash)]
enum RestrictionError {
	Mutability,
	ComplexPattern,
}

impl RestrictionError {
	pub const fn get_message(&self) -> &str {
		match self {
			RestrictionError::Mutability => LET_BINDING_MUTABILITY_ERR,
			RestrictionError::ComplexPattern => COMPLEX_PATTERN_ERR,
		}
	}
}

#[derive(Default)]
struct RestrictionErrors(Vec<(RestrictionError, Span)>);

impl RestrictionErrors {
	pub fn into_inner(self) -> Vec<(RestrictionError, Span)> {
		self.0
	}

	fn check_general_pattern(&mut self, pat: &Pat) {
		use RestrictionError::{ComplexPattern, Mutability};

		match pat {
			Pat::Ident(pat_ident) => {
				if pat_ident.mutability.is_none() {
					self.push((Mutability, pat.span()));
				};

				if let Some((_, subpat)) = &pat_ident.subpat {
					self.push((ComplexPattern, subpat.span()));
				}
			},
			Pat::Wild(_) => {
				// let that sink in
			},
			_ => self.push((ComplexPattern, pat.span())),
		}
	}
}

impl Deref for RestrictionErrors {
	type Target = Vec<(RestrictionError, Span)>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for RestrictionErrors {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<'a> Visit<'a> for RestrictionErrors {
	fn visit_local(&mut self, node: &'a syn::Local) {
		use RestrictionError::ComplexPattern;

		if let Some(init) = &node.init
			&& let Some(diverge) = &init.diverge
		{
			self.push((ComplexPattern, diverge.1.span()));
		}

		self.check_general_pattern(&node.pat);
		visit::visit_local(self, node);
	}

	fn visit_expr_for_loop(&mut self, node: &'a syn::ExprForLoop) {
		self.check_general_pattern(node.pat.as_ref());
		visit::visit_expr_for_loop(self, node);
	}
}

fn check_restriction_errors(karutin_fn: &KarutinFn) -> Option<Error> {
	let mut restriction_errors = RestrictionErrors::default();
	restriction_errors.visit_block(&karutin_fn.block);

	let errors = restriction_errors
		.into_inner()
		.into_iter()
		.map(|(err_type, span)| Error::new(span, err_type.get_message()));

	errors.reduce(|mut acc, error| {
		acc.combine(error);
		acc
	})
}

fn create_stack_generics(count: usize) -> TokenStream {
	let mut stream = TokenStream::new();

	for i in 0..count {
		let ty: Ident = format_generic_ident!(i);
		stream.extend(quote! { #ty, });
	}

	stream
}

fn create_empty_stack_generics(count: usize) -> TokenStream {
	let mut stream = TokenStream::new();

	for _ in 0..count {
		stream.extend(quote! { _, });
	}

	stream
}

fn create_stack_field_idents(count: usize) -> impl Iterator<Item = Ident> {
	(0..count).map(|i| format_field_ident!(i)).into_iter()
}

fn create_stack_fields(count: usize) -> TokenStream {
	let mut stream = TokenStream::new();

	for i in 0..count {
		let ident: Ident = format_field_ident!(i);
		let ty: Ident = format_generic_ident!(i);

		stream.extend(quote! {
			#ident: #ty,
		});
	}

	stream
}

#[derive(Default)]
struct Transpiler;

impl Transpiler {
	const SKIP_ATTR_STR: &str = "__skip_transpile__";
	const YIELD_FROM_ATTR_STR: &str = "__yield_from__";

	fn create_attr(ident: &str) -> Attribute {
		let _ident = Ident::new(ident, Span::mixed_site());
		parse_quote! { #[#_ident] }
	}

	pub fn create_skip_attr() -> Attribute {
		Self::create_attr(Self::SKIP_ATTR_STR)
	}

	pub fn create_yield_from_attr() -> Attribute {
		Self::create_attr(Self::YIELD_FROM_ATTR_STR)
	}

	fn get_attr_index(attrs: &Vec<Attribute>, ident: &str) -> Option<usize> {
		attrs.iter().enumerate().find_map(|(i, attr)| {
			attr.path().is_ident(ident);
			Some(i)
		})
	}

	fn remove_attr(attrs: &mut Vec<Attribute>, ident: &str) -> bool {
		if let Some(i) = Self::get_attr_index(attrs, ident) {
			attrs.remove(i);
			true
		} else {
			false
		}
	}

	fn remove_skip_attr(attrs: &mut Vec<Attribute>) -> bool {
		Self::remove_attr(attrs, Self::SKIP_ATTR_STR)
	}

	fn remove_yield_from_attr(attrs: &mut Vec<Attribute>) -> bool {
		Self::remove_attr(attrs, Self::YIELD_FROM_ATTR_STR)
	}

	//	for #pat# in #expr# #block#
	//
	//	  ||
	//	  ||
	//	\\||//
	//	 \\//
	//	  \/
	//
	//	{
	//		let mut iter = ::std::iter::IntoIterator::into_iter(#expr#);
	//		loop {
	//			let #pat# = match iter.next() {
	//				Some(v) => {v},
	//				None => break,
	//			}
	//			#block#
	//		}
	//	}
	fn transpile_for_loop(node: &mut ExprForLoop) -> ExprBlock {
		let pat = &node.pat;
		let expr = &node.expr;
		let body = &node.body;
		let label = node.label.as_ref();

		let skip_attr = Self::create_skip_attr();
		let mut for_loop_: ExprForLoop = parse_quote! {
			#skip_attr
			for _ in [(); 0] {}
		};

		let mut loop_: ExprLoop = parse_quote! {
			loop {
				let #pat = match iter.next() {
					Some(v) => {v},
					None => break,
				};
				#body
			}
		};

		if let Some(label) = label {
			loop_.label = Some(label.clone());
		}

		for_loop_.for_token.span = node.for_token.span;
		for_loop_.in_token.span = node.in_token.span;

		parse_quote! {{
			#for_loop_
			let mut iter = ::std::iter::IntoIterator::into_iter(#expr);
			#loop_
		}}
	}

	//	while #expr# #block#
	//
	//	  ||
	//	  ||
	//	\\||//
	//	 \\//
	//	  \/
	//
	//	{
	//		loop {
	//			if #expr# #block#
	//			else {
	//				break;
	//			}
	//		}
	//	}
	fn transpile_while_loop(node: &mut ExprWhile) -> ExprBlock {
		let expr = &node.cond;
		let body = &node.body;
		let label = node.label.as_ref();

		let skip_attr = Self::create_skip_attr();
		let mut while_loop_: ExprWhile = parse_quote! {
			#skip_attr
			while false {}
		};

		let mut loop_: ExprLoop = parse_quote! {
			loop {
				if #expr #body
				else {
					break;
				}
			}
		};

		if let Some(label) = label {
			loop_.label = Some(label.clone());
		}

		while_loop_.while_token.span = node.while_token.span;

		parse_quote! {{
			#while_loop_
			#loop_
		}}
	}

	//	#[__yield_from__]
	//	yield #expr#
	//
	//	  ||
	//	  ||
	//	\\||//
	//	 \\//
	//	  \/
	//
	//	{
	//		for val in ::karutin::into_value_iter!(#expr#) {
	//			yield val
	//		}
	//	}
	fn transpile_yield_from(node: &mut ExprYield) -> ExprBlock {
		let expr = &node.expr;

		parse_quote! {{
			for val in ::karutin::into_value_iter!(#expr) {
				yield val
			}
		}}
	}
}

impl VisitMut for Transpiler {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		match node {
			Expr::ForLoop(expr_for_loop) => {
				if Self::remove_skip_attr(&mut expr_for_loop.attrs) {
					return;
				};

				*node = Expr::Block(Self::transpile_for_loop(expr_for_loop));
				visit_mut::visit_expr_mut(self, node);
			},
			Expr::While(expr_while) => {
				*node = Expr::Block(Self::transpile_while_loop(expr_while));
				visit_mut::visit_expr_mut(self, node);
			},
			Expr::Yield(expr_yield) => {
				if Self::remove_yield_from_attr(&mut expr_yield.attrs) {
					*node = Expr::Block(Self::transpile_yield_from(expr_yield));
					visit_mut::visit_expr_mut(self, node);
				} else {
					visit_mut::visit_expr_yield_mut(self, expr_yield);
				}
			},
			_ => {},
		}
	}
}

fn transpile(node: &mut Block) {
	let mut transpiler = Transpiler::default();
	transpiler.visit_block_mut(node);
}

#[derive(Default)]
struct StateMachine {
	pub state_count: usize,
}

impl StateMachine {
	fn create_state_match_expr(&mut self, blocks: Vec<Block>) -> ExprMatch {
		let state_id = self.state_count;
		self.state_count += 1;

		let mut match_expr: ExprMatch = syn::parse_quote! {
			match self.states[#state_id] {}
		};

		attach_state_match_arms(state_id, &mut match_expr, blocks);

		match_expr
	}

	fn visit_block_stmts(&mut self, block: &mut Block) {
		for it in &mut block.stmts {
			self.visit_stmt_mut(it);
		}
	}
}

impl VisitMut for StateMachine {
	fn visit_expr_loop_mut(&mut self, node: &mut syn::ExprLoop) {
		let start = self.state_count;
		visit_mut::visit_expr_loop_mut(self, node);
		let end = self.state_count;

		let if_expr: ExprIf = syn::parse_quote! {
			if let Some(states) = self.states.get_mut(#start..#end) {
				states.fill(0);
			}
		};

		let expr = Expr::If(if_expr);
		let stmt = Stmt::Expr(expr, None);

		node.body.stmts.push(stmt);
	}

	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		match node {
			Expr::Yield(expr_yield) => {
				if let Some(expr) = &mut expr_yield.expr {
					self.visit_expr_mut(expr);
				}

				*node = Expr::Break(convert_yield(expr_yield));
			},
			Expr::Return(expr_return) => {
				if let Some(expr) = &mut expr_return.expr {
					self.visit_expr_mut(expr);
				}

				*node = Expr::Block(convert_return(expr_return));
			},
			_ => {},
		}

		visit_mut::visit_expr_mut(self, node);
	}

	fn visit_block_mut(&mut self, node: &mut syn::Block) {
		let stmts = std::mem::replace(&mut node.stmts, vec![]);
		let mut chunks = chunk_by_statefuls(stmts);

		if chunks.len() == 1 {
			if let Some(stmt) = chunks[0].last()
				&& !is_yield_stmt(stmt)
			{
				std::mem::swap(&mut node.stmts, &mut chunks[0]);

				self.visit_block_stmts(node);
				return;
			}
		}

		let mut blocks = chunks
			.into_iter()
			.map(|chunk| Block {
				brace_token: Default::default(),
				stmts: chunk,
			})
			.collect::<Vec<Block>>();

		let mut last_block = blocks.pop();

		if let Some(last_block_ref) = &last_block
			&& last_block_ref.stmts.len() == 1
			&& is_yield_stmt(&last_block_ref.stmts[0])
		{
			blocks.push(last_block.take().unwrap());
		}

		for mut block in blocks.iter_mut() {
			self.visit_block_stmts(&mut block);
		}

		let match_expr = self.create_state_match_expr(blocks);

		let expr = Expr::Match(match_expr);
		let stmt = Stmt::Expr(expr, None);

		node.stmts.push(stmt);

		if let Some(mut last_block) = last_block {
			self.visit_block_stmts(&mut last_block);

			let block_expr = ExprBlock {
				attrs: vec![],
				label: None,
				block: last_block,
			};

			let expr = Expr::Block(block_expr);
			let stmt = Stmt::Expr(expr, None);

			node.stmts.push(stmt);
		}
	}
}

fn sift_states(node: &mut Block) -> usize {
	let mut state_machine = StateMachine::default();

	state_machine.state_count += 1;
	state_machine.visit_block_mut(node);

	state_machine.state_count
}

#[derive(Default)]
struct StackScope(HashMap<String, usize>);

#[derive(Default)]
struct StackBuilder {
	pub scopes: Vec<StackScope>,
	pub local_count: usize,
}

impl StackBuilder {
	fn lookup_local(&self, ident: &Ident) -> Option<usize> {
		let ident_str = ident.to_string();
		let mut result = Option::<usize>::None;

		for scope in self.scopes.iter().rev() {
			if result.is_some() {
				break;
			}

			let ret = scope.0.get(&ident_str);

			if let Some(id) = ret {
				result = Some(*id);
			}
		}

		result
	}

	fn insert_local(&mut self, ident: &Ident) -> usize {
		let ident_str = ident.to_string();
		let result = self.local_count;

		let last = self.scopes.last_mut().unwrap();

		last.0.insert(ident_str, result);

		self.local_count += 1;
		result
	}

	fn convert_expr(&self, expr: &mut Expr) -> bool {
		if let Expr::Path(expr_path) = expr
			&& expr_path.path.segments.len() == 1
		{
			let ident = &expr_path.path.segments[0].ident;

			if let Some(id) = self.lookup_local(ident) {
				let mut field_ident = format_field_ident!(id);
				field_ident.set_span(ident.span());

				let new_expr: Expr = parse_quote_spanned! {ident.span()=>
					stack.#field_ident
				};

				*expr = new_expr;
				return true;
			}
		}

		false
	}
}

impl VisitMut for StackBuilder {
	fn visit_expr_mut(&mut self, node: &mut Expr) {
		if !self.convert_expr(node) {
			visit_mut::visit_expr_mut(self, node);
		}
	}

	fn visit_local_mut(&mut self, node: &mut Local) {
		if let Pat::Ident(pat_ident) = &mut node.pat {
			let id = self.insert_local(&pat_ident.ident);
			let ident_span = pat_ident.ident.span();

			pat_ident.ident = Ident::new(LET_BINDING_IDENT_STR, ident_span);

			if let Some(init) = &mut node.init {
				let mut field_ident = format_field_ident!(id);
				field_ident.set_span(ident_span);

				let expr = &init.expr;
				let block_expr: ExprBlock = parse_quote_spanned! {ident_span=>{
						stack.#field_ident = #expr
				}};

				init.expr = Box::new(block_expr.into());
				self.visit_expr_mut(&mut init.expr);
			}
		} else {
			visit_mut::visit_local_mut(self, node);
		}
	}

	fn visit_block_mut(&mut self, node: &mut Block) {
		self.scopes.push(Default::default());
		visit_mut::visit_block_mut(self, node);
		self.scopes.pop();
	}
}

fn build_stack(node: &mut Block) -> usize {
	let mut builder = StackBuilder::default();

	builder.visit_block_mut(node);

	builder.local_count
}

struct KarutinReturnType {
	pub yield_type: Box<Type>,
	pub return_type: Box<Type>,
}

impl Parse for KarutinReturnType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			yield_type: {
				input.parse::<Token![->]>()?;
				input.parse()?
			},
			return_type: {
				input.parse::<Token![..]>()?;
				input.parse()?
			},
		})
	}
}

impl ToTokens for KarutinReturnType {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let yield_type = &self.yield_type;
		let return_type = &self.return_type;

		let args: Punctuated<GenericArgument, Token![,]> = parse_quote! {
			Yield = #yield_type,
			Return = #return_type
		};

		args.to_tokens(tokens);
	}
}

struct KarutinParameters {
	pub paren_token: Paren,
	pub inputs: Punctuated<FnArg, Comma>,
}

impl KarutinParameters {
	pub fn into_pat_type(self) -> PatType {
		let inputs_iter = self.inputs.into_iter();

		let fm_closure = |arg| match arg {
			FnArg::Typed(pt) => Some((pt.pat, pt.ty)),
			_ => None,
		};

		let pairs: (Vec<Box<Pat>>, Vec<Box<Type>>) = inputs_iter.filter_map(fm_closure).unzip();

		let (pats, types) = pairs;

		parse_quote! {
			( #( #pats ),* ): ( #( #types ),* )
		}
	}
}

impl Parse for KarutinParameters {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;

		Ok(Self {
			paren_token: parenthesized!(content in input),
			inputs: content.parse_terminated(FnArg::parse, Token![,])?,
		})
	}
}

impl ToTokens for KarutinParameters {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.paren_token.surround(tokens, |tokens| {
			self.inputs.to_tokens(tokens);
		});
	}
}

struct KarutinSignature {
	pub unsafety: Option<Unsafe>,
	pub fn_token: Token![fn],
	pub ident: Ident,
	pub generics: Generics,
	pub parameters: KarutinParameters,
	pub output: KarutinReturnType,
}

impl Parse for KarutinSignature {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			unsafety: input.parse()?,
			fn_token: input.parse()?,
			ident: input.parse()?,
			generics: input.parse()?,
			parameters: input.parse()?,
			output: input.parse()?,
		})
	}
}

impl ToTokens for KarutinSignature {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.unsafety.to_tokens(tokens);
		self.fn_token.to_tokens(tokens);
		self.ident.to_tokens(tokens);
		self.generics.to_tokens(tokens);
		self.parameters.to_tokens(tokens);

		let type_stream = &mut TokenStream::new();

		self.parameters.to_tokens(type_stream);
		Comma::default().to_tokens(type_stream);
		self.output.to_tokens(type_stream);

		let type_: Type = parse_quote! {
			impl ::karutin::Karutin<#type_stream>
		};

		RArrow::default().to_tokens(tokens);
		type_.to_tokens(tokens);
	}
}

struct KarutinFn {
	pub vis: Visibility,
	pub sig: KarutinSignature,
	pub block: Box<Block>,
}

impl Parse for KarutinFn {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			vis: input.parse()?,
			sig: input.parse()?,
			block: input.parse()?,
		})
	}
}

impl ToTokens for KarutinFn {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.vis.to_tokens(tokens);
		self.sig.to_tokens(tokens);
		self.block.to_tokens(tokens);
	}
}

struct KarutinFnList {
	inner: Vec<KarutinFn>,
}

impl KarutinFnList {
	fn into_inner(self) -> Vec<KarutinFn> {
		self.inner
	}
}

impl Parse for KarutinFnList {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut inner = Vec::new();

		while !input.is_empty() {
			inner.push(input.parse()?);
		}

		Ok(Self { inner })
	}
}

impl ToTokens for KarutinFnList {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		for v in &self.inner {
			v.to_tokens(tokens);
		}
	}
}

struct KarutinSigList {
	inner: Vec<KarutinSignature>,
}

impl KarutinSigList {
	fn into_inner(self) -> Vec<KarutinSignature> {
		self.inner
	}
}

impl Parse for KarutinSigList {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut inner = Vec::new();

		while !input.is_empty() {
			inner.push(input.parse()?);
			let _: Semi = input.parse()?;
		}

		Ok(Self { inner })
	}
}

impl ToTokens for KarutinSigList {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		for v in &self.inner {
			v.to_tokens(tokens);
			Semi::default().to_tokens(tokens);
		}
	}
}

enum Karutin {
	DefinitionList(KarutinFnList),
	DeclarationList(KarutinSigList),
}

impl Parse for Karutin {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut errors: Option<Error> = None;

		let mut combine = |e: Error| {
			let errors = &mut errors;

			if let Some(errors) = errors {
				errors.combine(e);
			} else {
				let _ = errors.insert(e);
			}
		};

		let fork = &input.fork();
		match KarutinFnList::parse(fork) {
			Ok(v) => {
				input.advance_to(fork);
				return Ok(Self::DefinitionList(v));
			},
			Err(e) => combine(e),
		}

		let fork = &input.fork();
		match KarutinSigList::parse(fork) {
			Ok(v) => {
				input.advance_to(fork);
				return Ok(Self::DeclarationList(v));
			},
			Err(e) => combine(e),
		}

		Err(errors.unwrap())
	}
}

impl ToTokens for Karutin {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Karutin::DefinitionList(karutin_fn_list) => karutin_fn_list.to_tokens(tokens),
			Karutin::DeclarationList(karutin_sig_list) => karutin_sig_list.to_tokens(tokens),
		}
	}
}

fn wrap_completed_state(body: Box<Block>) -> TokenStream {
	parse_quote! {
		if self.states[#COMPLETED_STATE_ID] == 0 {
			#[allow(unreachable_code)]
			let _state = ::karutin::KarutinState::Returned( #body );

			self.states[#COMPLETED_STATE_ID] = 1;
			_state
		} else {
			::karutin::KarutinState::Completed
		}
	}
}

fn zeroed_stack_locals(local_count: usize) -> TokenStream {
	let fields = create_stack_field_idents(local_count);

	quote! {
		#(
			stack.#fields = ::karutin::internal::unchecked_zeroed();
		)*
	}
}

fn handle_moved_stack(local_count: usize) -> TokenStream {
	let zsl = zeroed_stack_locals(local_count);
	let swap = quote! {
		unsafe {
			::std::mem::swap(
				&mut *raw_stack_ptr,
				&mut stack_rep,
			);
		}
	};

	quote! { #swap #zsl #swap }
}

fn obtain_default_lifetime(ty: &mut Type) {
	match ty {
		Type::Array(type_array) => obtain_default_lifetime(type_array.elem.as_mut()),
		Type::Group(type_group) => obtain_default_lifetime(type_group.elem.as_mut()),
		Type::Paren(type_paren) => obtain_default_lifetime(type_paren.elem.as_mut()),
		Type::Slice(type_slice) => obtain_default_lifetime(type_slice.elem.as_mut()),
		Type::Path(_type_path) => {
			// IDK
		},
		Type::Tuple(type_tuple) => {
			for ty in type_tuple.elems.iter_mut() {
				obtain_default_lifetime(ty);
			}
		},
		Type::Reference(type_reference) if type_reference.lifetime.is_none() => {
			let lifetime = Lifetime::new(LIFETIME_STR, Span::call_site());
			type_reference.lifetime = Some(lifetime);
		},
		_ => {},
	}
}

fn wrap_stack_management(
	stack_ident: &Ident,
	empty_generics: &TokenStream,
	local_count: usize,
	body: TokenStream,
) -> TokenStream {
	let hms = handle_moved_stack(local_count);
	let state_loop_label = Lifetime::new(STATE_LOOP_LABEL_STR, Span::call_site());

	quote! {
		let mut stack;
		let mut stack_rep;

		if let Some(stack_) = self.stack.as_ref() {
			(stack, stack_rep) = stack_.get_boxes::<#stack_ident<#empty_generics>>();
		} else {
			(stack, stack_rep) = ::karutin::internal::KarutinStack::create_zeroeds();
			let ret = ::karutin::internal::KarutinStack::from((stack, stack_rep));
			return ::karutin::internal::KarutinResponse::StackExpose(ret);
		}

		let raw_stack_ptr = &mut stack as *mut Box<_>;

		let ret = #state_loop_label: loop {
			break #body
		};

		#hms

		::std::mem::forget(stack);
		::std::mem::forget(stack_rep);

		::karutin::internal::KarutinResponse::StateLoop(ret)
	}
}

fn check_karutin_fn(karutin_fn: &KarutinFn) -> Option<Error> {
	let checks = [
		check_blocks_macro_usage(karutin_fn),
		check_restriction_errors(karutin_fn),
	];

	let mut checks_result = Option::<Error>::None;

	for check in checks.into_iter() {
		match (&mut checks_result, check) {
			(None, Some(err)) => {
				checks_result = Some(err);
			},
			(Some(base_err), Some(err)) => {
				base_err.combine(err);
			},
			_ => {},
		}
	}

	checks_result
}

fn karutin_stack(ident: &Ident, generics: &TokenStream, fields: &TokenStream) -> TokenStream {
	quote! {
		#[allow(non_camel_case_types)]
		struct #ident<#generics> {
			#fields
		}
	}
}

fn karutin_ctx(ident: &Ident, lifetime: &Lifetime, state_count: usize) -> TokenStream {
	quote! {
		#[allow(non_camel_case_types)]
		#[derive(Default)]
		struct #ident<#lifetime> {
			stack: Option<::karutin::internal::KarutinStack<#lifetime>>,
			states: [usize; #state_count]
		}
	}
}

fn karutin_resume_inner(
	ctx_ident: &Ident,
	lifetime: &Lifetime,
	generics: &Generics,
	params_pat: &TokenStream,
	params_ty: &Type,
	yield_type: &Type,
	return_type: &Type,
	body: &TokenStream,
) -> TokenStream {
	quote! {
		#[allow(unused_braces)]
		impl<#lifetime> #ctx_ident<#lifetime> {
			fn resume_inner #generics (&mut self, #params_pat: #params_ty)
				-> ::karutin::internal::KarutinResponse<#lifetime, #yield_type, #return_type>
			{
				#body
			}
		}
	}
}

fn karutin_impl_debug(ctx_ident: &Ident, lifetime: &Lifetime, debug_name: &String) -> TokenStream {
	quote! {
		impl<#lifetime> std::fmt::Debug for #ctx_ident<#lifetime> {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct(#debug_name)
					.field("stack", self.stack.as_ref().unwrap())
					.field("states", &self.states)
					.finish()
			}
		}
	}
}

fn karutin_impl_karutin(
	ctx_ident: &Ident,
	lifetime: &Lifetime,
	generics: &Generics,
	parameters_ty: &Type,
	yield_type: &Type,
	return_type: &Type,
) -> TokenStream {
	quote! {
		impl #generics ::karutin::Karutin<#parameters_ty> for #ctx_ident<#lifetime> {
			type Yield = #yield_type;
			type Return = #return_type;

			#[inline(always)]
			fn resume(
				&mut self,
				args: #parameters_ty
			) -> ::karutin::KarutinState<#yield_type, #return_type> {
				match self.resume_inner(args) {
					::karutin::internal::KarutinResponse::StateLoop(v) => v,
					_ => { unreachable!() },
				}
			}
		}
	}
}

fn karutin_signature(
	ident: &Ident,
	unsafety: &Option<Unsafe>,
	generics: &Generics,
	parameters_ty: &Type,
	yield_type: &Type,
	return_type: &Type,
) -> TokenStream {
	quote! {
		#unsafety fn #ident #generics () -> impl ::karutin::Karutin<
			#parameters_ty,
			Yield = #yield_type,
			Return = #return_type
		>
	}
}

fn _karutin_fn(
	ctx_ident: &Ident,
	ident: &Ident,
	vis: &Visibility,
	unsafety: &Option<Unsafe>,
	generics: &Generics,
	parameters_ty: &Type,
	yield_type: &Type,
	return_type: &Type,
) -> TokenStream {
	let signature = karutin_signature(
		ident,
		unsafety,
		generics,
		parameters_ty,
		yield_type,
		return_type,
	);

	quote! {
		#[inline]
		#vis #signature {
			let mut ctx = #ctx_ident::default();

			let cold_start = ctx.resume_inner(
				::karutin::internal::unchecked_zeroed()
			);

			match cold_start {
				::karutin::internal::KarutinResponse::StackExpose(v) => {
					ctx.stack = Some(v);
				},
				_ => { unreachable!() },
			}

			ctx
		}
	}
}

fn karutin_definition(mut karutin_fn: KarutinFn) -> TokenStream {
	if let Some(failed_check) = check_karutin_fn(&karutin_fn) {
		return failed_check.into_compile_error();
	}

	transpile(&mut karutin_fn.block);

	let local_count = build_stack(&mut karutin_fn.block);
	let state_count = sift_states(&mut karutin_fn.block);

	let vis = karutin_fn.vis;
	let unsafety = karutin_fn.sig.unsafety;
	let ident = karutin_fn.sig.ident;

	let ctx_ident = format_context_ident!(ident);
	let lifetime = Lifetime::new(LIFETIME_STR, Span::call_site());

	let generics = karutin_fn.sig.generics;
	let mut combined_generics = generics.clone();
	let mut inner_generics = generics.clone();

	let lifetime_param = LifetimeParam::new(lifetime.clone());
	let generic_param = GenericParam::Lifetime(lifetime_param);

	combined_generics.params.insert(0, generic_param);

	for generic_param in &mut inner_generics.params {
		match generic_param {
			syn::GenericParam::Lifetime(lifetime_param) => {
				lifetime_param.bounds.push(lifetime.clone());
			},
			_ => {},
		}
	}

	let parameters = karutin_fn.sig.parameters.into_pat_type();
	let parameters_pat = parameters.pat.to_token_stream();

	let mut parameters_ty = parameters.ty;
	let mut yield_type = karutin_fn.sig.output.yield_type;
	let mut return_type = karutin_fn.sig.output.return_type;

	obtain_default_lifetime(parameters_ty.as_mut());
	obtain_default_lifetime(yield_type.as_mut());
	obtain_default_lifetime(return_type.as_mut());

	let body = wrap_completed_state(karutin_fn.block).to_token_stream();

	let stack_ident = format_stack_ident!(ident);
	let stack_generics = create_stack_generics(local_count);
	let empty_stack_generics = create_empty_stack_generics(local_count);
	let stack_fields = create_stack_fields(local_count);

	let body2 = wrap_stack_management(&stack_ident, &empty_stack_generics, local_count, body);

	let debug_name = format!("Karutin Context ({})", ident);

	let stack_quote = karutin_stack(&stack_ident, &stack_generics, &stack_fields);
	let ctx_quote = karutin_ctx(&ctx_ident, &lifetime, state_count);
	let resume_inner_quote = karutin_resume_inner(
		&ctx_ident,
		&lifetime,
		&inner_generics,
		&parameters_pat,
		&parameters_ty,
		&yield_type,
		&return_type,
		&body2,
	);
	let impl_debug_quote = karutin_impl_debug(&ctx_ident, &lifetime, &debug_name);
	let impl_karutin = karutin_impl_karutin(
		&ctx_ident,
		&lifetime,
		&combined_generics,
		&parameters_ty,
		&yield_type,
		&return_type,
	);
	let _fn = _karutin_fn(
		&ctx_ident,
		&ident,
		&vis,
		&unsafety,
		&combined_generics,
		&parameters_ty,
		&yield_type,
		&return_type,
	);

	quote! {
		#stack_quote
		#ctx_quote
		#resume_inner_quote
		#impl_debug_quote
		#impl_karutin
		#_fn
	}
}

fn karutin_declaration(karutin_sig: KarutinSignature) -> TokenStream {
	let unsafety = karutin_sig.unsafety;
	let ident = karutin_sig.ident;

	let mut combined_generics = karutin_sig.generics;

	let lifetime = Lifetime::new(LIFETIME_STR, Span::call_site());
	let lifetime_param = LifetimeParam::new(lifetime.clone());
	let generic_param = GenericParam::Lifetime(lifetime_param);
	let parameters = karutin_sig.parameters.into_pat_type();

	let mut parameters_ty = parameters.ty;
	let mut yield_type = karutin_sig.output.yield_type;
	let mut return_type = karutin_sig.output.return_type;

	combined_generics.params.insert(0, generic_param);

	obtain_default_lifetime(parameters_ty.as_mut());
	obtain_default_lifetime(yield_type.as_mut());
	obtain_default_lifetime(return_type.as_mut());

	let signature = karutin_signature(
		&ident,
		&unsafety,
		&combined_generics,
		&parameters_ty,
		&yield_type,
		&return_type,
	);

	quote! {
		#signature;
	}
}

type KarutinDslInput<'a> = &'a mut Peekable<proc_macro::token_stream::IntoIter>;

fn karutin_dsl_yield_from(input: KarutinDslInput, output: &mut proc_macro::TokenStream) {
	let yield_ident = match input.next() {
		Some(proc_macro::TokenTree::Ident(i)) => i,
		_ => unreachable!(),
	};

	let attr = Transpiler::create_yield_from_attr();
	let attr_stream2: TokenStream = quote_spanned! {yield_ident.span().into()=>#attr};
	let attr_stream: proc_macro::TokenStream = attr_stream2.into_token_stream().into();

	output.extend(attr_stream);
	output.extend([proc_macro::TokenTree::Ident(yield_ident)]);
}

fn karutin_dsl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut output = proc_macro::TokenStream::new();
	let mut input = input.into_iter().peekable();

	while let Some(tt) = input.next() {
		match tt {
			proc_macro::TokenTree::Punct(p) if p.as_char() == '~' => {
				if let Some(proc_macro::TokenTree::Ident(ident)) = input.peek() {
					if ident.to_string() == "yield" {
						karutin_dsl_yield_from(&mut input, &mut output);
						continue;
					}
				}

				output.extend([p]);
			},
			proc_macro::TokenTree::Group(g) => {
				let del = g.delimiter();
				let stream = karutin_dsl(g.stream());
				let group = proc_macro::Group::new(del, stream);

				output.extend([group]);
			},
			other => output.extend([other]),
		}
	}

	output
}

/// *The beginning of everything.*
///
/// <ins>Before using this, be sure to read the [Memory](#-memory) section!</ins>
///
/// Makes it easy to use [`karutin`](index.html)
/// 
/// You just need to write the coroutine, this macro handles:
/// - Stack management
/// - State machine
/// - Yield mechanism
/// 
/// After all this, you call the corotutine
/// and it returns a auto-generated struct
/// that implements [`Karutin`](trait.Karutin.html) by your coroutine declaration.
/// 
/// # Example
/// Simple example for getting fibonacci sequence
/// ```
/// use karutin::{KarutinGen, KarutinState, karutin};
/// 
/// karutin! {
/// 	pub fn fibonacci() -> usize..() {
/// 		let mut a = 0;
/// 		let mut b = 1;
/// 
/// 		for _ in 0..10 {
/// 			yield a;
/// 			let mut next = a + b;
/// 			a = b;
/// 			b = next;
/// 		}
/// 	}
/// }
/// 
/// fn main() {
/// 	let mut fibonacci_seq = fibonacci();
/// 
/// 	assert_eq!(KarutinState::Yielded(0), fibonacci_seq.resume());
/// 	assert_eq!(KarutinState::Yielded(1), fibonacci_seq.resume());
/// 	assert_eq!(KarutinState::Yielded(1), fibonacci_seq.resume());
/// 	assert_eq!(KarutinState::Yielded(2), fibonacci_seq.resume());
/// 	assert_eq!(KarutinState::Yielded(3), fibonacci_seq.resume());
/// }
/// ```
///
/// # Input
///
/// This macro accept two type of input currently:
/// - Declaration List
/// ```ignore
/// karutin! {
/// 	pub a() -> ()..();
///		...
/// }
/// ```
///
/// - Definition List
/// ```ignore
/// karutin! {
/// 	pub a() -> ()..() {
/// 		yield;
/// 	}
/// 	...
/// }
/// ```
///
/// ## DSL
/// ### "yield":
/// If you wanna pause the coroutine execution and return a value, you "yield".
/// To get the value and continue the execution, you call
/// [`Karutin::resume`](trait.Karutin.html#tymethod.resume) method.
// TODO
/// ```ignore
/// karutin! {
/// 	pub a() -> usize..() {
/// 		yield 1;
/// 	}
/// }
/// ```
/// ### "~yield":
/// Does the same thing as the "yield" but this can "yield" all values
/// from a iterator or another coroutine.
// TODO
/// ```ignore
/// karutin! {
/// 	pub a() -> usize..() {
/// 		~yield [1, 2];
/// 	}
/// }
/// ```
/// ### Function signature:
/// The function signatures in the input must follow this `Function` rule
/// ([see the details][fn_syntax_ref]):
///
/// [fn_syntax_ref]: https://doc.rust-lang.org/reference/items/functions.html
///
/// ```ignore
/// YieldType -> Type
/// ReturnType -> Type
/// Function -> "unsafe"? "fn" IDENTIFIER GenericParams? ( FunctionParameters? ) "->" YieldType ".." ReturnType
/// ```
///
/// # Output
/// For declaration list, the macro just converts signatures
/// to implement [`Karutin`](trait.Karutin.html) trait,
/// and makes the references obtain the karutin lifetime[^karutin_lifetime_note].
///
///	[^karutin_lifetime_note]: "Karutin lifetime" is
/// the default lifetime that getting obtained automaticly
/// by reference types which do not have explicit lifetimes.
/// The reason is associating references with karutin context.
///
/// For definition list, the macro does what it does for a declaration list,
/// but also do these few things:
/// - Convert the function body into state machine by the stateful points, like:
/// 	- yields,
/// 	- loops,
/// 	- potantial yields (any expression that contain another stateful point in deep) \[WIP\],
/// 	- conditional statements \[WIP\],
/// - Cover stack management with:
/// 	- creating generic stack struct for storing locals,
/// 	- generate a resistant mechanism to
/// 		- handle loading and saving the stack
/// 		- moving fields from the stack
/// 	- ensure all of this working with inference
///
/// # ⚠️ Memory
///	**KARUTIN IS NOT CABAPLE TO MANAGE DROPPING YET!!!**
///
/// Karutin manages its own variable stack,
/// and because of following variables in macro context
/// to when they are moved and out-of-scoped is are hard,
/// there is no way to tell when one variable must be droped.
///
/// This is not a problem for [Copy](https://doc.rust-lang.org/stable/core/marker/trait.Copy.html) values,
/// eventually they do not need to [Drop](https://doc.rust-lang.org/stable/core/ops/trait.Drop.html),
/// and they will be erased when value stack dropped.
///
/// Even [Drop](https://doc.rust-lang.org/stable/core/ops/trait.Drop.html)
/// values will be erased but this is just a bitwise operation,
/// if there is a logic based on drop, it will collapse;
/// if there are heap allocated values managed by the erased value, they will be leaked.
///
/// So we strictly recommend using references in most case,
/// but if you inevitably need a owned value that must be dropped,
/// drop it manualy by the flow while keeping the fact that
/// "coroutines may not be fully resumed" in mind!
///
/// ```ignore
/// karutin! {
/// 	pub a(value: Box<_>) -> ()..() {
/// 		//^^^^^ Inner value leaked
/// 		yield;
/// 	}
///
/// 	pub b(value: Arc<_>) -> ()..() {
/// 		//^^^^^ Reference count is not decremented
/// 		yield;
/// 	}
///
/// 	pub c() -> ()..() {
/// 		let mut val = /* any value that must be droped */;
/// 			  //^^^ Not dropped
/// 		yield;
/// 	}
/// }
/// ```
///
/// # Panics
/// If the warnings in [Memory](#-memory) section had been taken into the account,
/// there is only one known way to make a Karutin panicked: Faulty code generation.
/// 
/// Because of faulty code generation,
/// program may fall into an area that labeled as
/// [`unreachable`](https://doc.rust-lang.org/std/macro.unreachable.html)
/// and panicked
/// 
/// If this is happening, please report this on the issue tracker!
#[proc_macro]
pub fn karutin(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = karutin_dsl(input);
	let parsed = parse_macro_input!(input as Karutin);

	let mut stream = TokenStream::new();

	match parsed {
		Karutin::DefinitionList(karutin_fn_list) => {
			for karutin_fn in karutin_fn_list.into_inner() {
				stream.extend(karutin_definition(karutin_fn));
			}
		},
		Karutin::DeclarationList(karutin_sig_list) => {
			for karutin_sig in karutin_sig_list.into_inner() {
				stream.extend(karutin_declaration(karutin_sig));
			}
		},
	}

	stream.into_token_stream().into()
}

/// *The beginning of everything,* but as [`&str`].
///
/// Processes input with [`karutin!`](macro@karutin),
/// converts the output into [`&str`],
/// format the syntax of it with [`prettyplease`][prettyplease],
/// and finally streams it as string literal.
///
/// [prettyplease]: https://docs.rs/prettyplease/latest/prettyplease/
///
/// For informatin about processing, please refer to [karutin!](karutin)
#[proc_macro]
pub fn karutin_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let stream = karutin(input);

	let syntax_tree: syn::File = parse2(stream.into()).unwrap();
	let formatted = prettyplease::unparse(&syntax_tree);

	let str: Expr = parse_quote! { #formatted };

	str.into_token_stream().into()
}
