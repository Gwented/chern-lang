// // Linters are annoying.
// use common::{
//     intern::Intern,
//     symbols::{InnerArgs, SpannedInnerArgs},
// };
//
// use crate::parser::ast::{
//     AbstractTypeDef, AbstractVariant, AstInfo, Expr, Item, SpannedExpr, TypeExpr,
// };
//
// //WARN: FOR SANITY PURPOSES
// pub fn print_all(ast_info: &AstInfo, interner: &Intern) {
//     let indent = 4;
//     let spaces = " ".repeat(indent);
//
//     if let Some(name_id) = ast_info.bind {
//         let name = interner.search(name_id.id as usize);
//
//         println!("bind = \"{name}\"");
//     }
//
//     for item in &ast_info.items {
//         match item {
//             Item::Var(ty) => {
//                 let name = interner.search(ty.name_id.id as usize);
//                 println!("TypeDef {name} [");
//                 print_type(&ty.ty_expr, indent + 2, interner);
//
//                 print_exprs(&ty.conds, indent + 2, interner);
//
//                 println!("]");
//             }
//             Item::Struct(structure) => {
//                 let name = interner.search(structure.name_id.id as usize);
//                 println!("Struct {name} [");
//
//                 for ty in &structure.fields {
//                     let temp_indent = indent + 2;
//
//                     let temp_spaces = " ".repeat(temp_indent);
//
//                     let name = interner.search(ty.name_id.id as usize);
//
//                     println!("{temp_spaces}{name}");
//
//                     print_type(&ty.ty_expr, temp_indent, interner);
//
//                     print_exprs(&ty.conds, temp_indent, interner);
//
//                     print_args(&ty.args, temp_indent, interner);
//                 }
//
//                 print_exprs(&structure.glob_conds, indent, interner);
//
//                 print_args(&structure.glob_args, indent, interner);
//
//                 println!("]");
//             }
//             Item::Enum(enumeration) => {
//                 let name = interner.search(enumeration.name_id.id as usize);
//                 println!("Enum {name} [");
//
//                 print_variants(&enumeration.variants, indent, interner);
//                 print_args(&enumeration.glob_args, indent, interner);
//                 print_exprs(&enumeration.glob_conds, indent, interner);
//
//                 println!("]");
//             }
//             Item::Alias(abs_alias) => {
//                 let name = interner.search(abs_alias.name_id.id as usize);
//                 println!("Alias {name} [");
//
//                 for ty_expr in &abs_alias.params {
//                     print_type(ty_expr, indent + 2, interner);
//                 }
//                 print_exprs(&abs_alias.conds, indent + 2, interner);
//                 print_args(&abs_alias.args, indent + 2, interner);
//
//                 println!("]");
//             }
//             Item::Const(abs_const) => {
//                 let name = interner.search(abs_const.name_id.id as usize);
//                 println!("Const {name} [");
//             }
//             Item::Import(abstract_import) => (),
//         }
//     }
//     println!("]");
// }
//
// fn print_type(ty: &TypeExpr, indent: usize, interner: &Intern) {
//     let spaces = " ".repeat(indent);
//     match ty {
//         TypeExpr::Var(name_id, _) | TypeExpr::Escaped(name_id, _) => {
//             let type_name = interner.search(name_id.id as usize);
//             println!("{spaces}type: {type_name}");
//         }
//         TypeExpr::Generic(generic, _) => {
//             let base_name = interner.search(generic.base.id as usize);
//             println!("{spaces}generic: {base_name} [");
//             print_generic(&generic.args, indent + 2, interner);
//             println!("{spaces}]");
//         }
//         TypeExpr::Any(_) => {
//             println!("{spaces}Any");
//         }
//         TypeExpr::Tuple(ty_exprs, _) => {
//             println!("{spaces}tuple:");
//             for thing in ty_exprs {
//                 print_type(thing, indent + 2, interner);
//             }
//             println!("{spaces}]");
//         }
//     }
// }
//
// //WARN: Did not properly create recursive this.doThat() entry point
// fn print_fields(fields: &Vec<AbstractTypeDef>, indent: usize, interner: &Intern) {
//     for ty in fields {}
// }
//
// fn print_variants(variants: &Vec<AbstractVariant>, indent: usize, interner: &Intern) {
//     let spaces = " ".repeat(indent);
//
//     for variant in variants {
//         let name = interner.search(variant.name_id.id as usize);
//         println!("{spaces}Variant: {name}");
//
//         if let Some(ty) = &variant.ty {
//             print_type(ty, indent, interner);
//             println!();
//         }
//
//         print_exprs(&variant.conds, indent, interner);
//         print_args(&variant.args, indent, interner);
//     }
// }
//
// fn print_generic(args: &Vec<TypeExpr>, indent: usize, interner: &Intern) {
//     for ty in args {
//         print_type(ty, indent, interner);
//     }
// }
//
// fn print_exprs(conds: &Vec<SpannedExpr>, indent: usize, interner: &Intern) {
//     let spaces = " ".repeat(indent);
//
//     // They're unresolvedddddddddd THEY'RE UNRESOLVED
//     // BUT I NEED TO KNOW
//     for spanned_expr in conds {
//         match &spanned_expr.expr {
//             Expr::Var(name_id) => {
//                 let name = interner.search(name_id.id as usize);
//                 println!("{spaces}condition: {name}")
//             }
//             Expr::Integer(num) => {
//                 println!("{spaces}number: {num}")
//             }
//             Expr::Str(name_id) => {
//                 let name = interner.search(name_id.id as usize);
//                 println!("{spaces}{name}")
//             }
//             Expr::Call(_, _) => todo!(),
//             Expr::Unary(unary) => {
//                 println!("{spaces}Unary [");
//                 println!("{spaces}{:?}", unary.op);
//
//                 if let Expr::Var(name_id) = *&unary.spanned_expr.expr {
//                     let name = interner.search(name_id.id as usize);
//                     println!("{spaces}{name}");
//                 }
//
//                 println!("{spaces}]");
//             }
//             Expr::FieldAccess(field_access) => {
//                 println!("{spaces}FieldAccess [");
//                 let field_name = interner.search(field_access.field.id as usize);
//
//                 println!("{spaces}{field_name}");
//                 print_exprs(conds, indent, interner);
//
//                 println!("{spaces}]");
//             }
//             Expr::Float(num) => {
//                 println!("{spaces}float: {num}");
//             }
//             Expr::BinaryExpr { lhs, op, rhs } => todo!(),
//             Expr::Char(ch) => {
//                 println!("{spaces}Char [{ch}");
//             }
//             Expr::Default(name_id, _) => {
//                 println!("{spaces}Default [");
//
//                 let name = interner.search(name_id.id as usize);
//                 println!("{spaces}{name}");
//
//                 println!("{spaces}]");
//             }
//         }
//     }
// }
//
// fn print_args(args: &Vec<SpannedInnerArgs>, indent: usize, interner: &Intern) {
//     let spaces = " ".repeat(indent);
//
//     let other_spaces = " ".repeat(indent + 2);
//
//     if !args.is_empty() {
//         println!("{spaces}Args [");
//         for arg in args {
//             println!("{other_spaces}{arg:?}");
//         }
//         println!("{spaces}]");
//     }
// }
