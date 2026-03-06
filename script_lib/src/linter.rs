use common::{
    intern::Intern,
    symbols::{Cond, InnerArgs},
};

use crate::parser::ast::{AbstractType, Expr, Generic, Item, TypeExpr, Variant};

//WARN: FOR SANITY PURPOSES
pub fn print_all(ast: &Vec<Item>, interner: &Intern) {
    let indent = 4;
    let spaces = " ".repeat(indent);

    for item in ast {
        match item {
            Item::Bind(bind) => {
                let name = interner.search(bind.name_id.id as usize);
                println!("Bind [\n{spaces}{name}");
                println!("]");
            }
            Item::Var(ty) => {
                let name = interner.search(ty.name_id.id as usize);
                println!("TypeDef {name} [");
                print_type(&ty.ty, indent + 2, interner);

                print_exprs(&ty.conds, indent + 2, interner);

                println!("]");
            }
            Item::Struct(structure) => {
                let name = interner.search(structure.name_id.id as usize);
                println!("Struct {name} [");

                for ty in &structure.fields {
                    let temp_indent = indent + 2;

                    let temp_spaces = " ".repeat(temp_indent);

                    let name = interner.search(ty.name_id.id as usize);

                    println!("{temp_spaces}{name}");

                    print_type(&ty.ty, temp_indent, interner);

                    print_exprs(&ty.conds, temp_indent, interner);

                    print_args(&ty.args, temp_indent, interner);
                }

                print_exprs(&structure.conds, indent, interner);

                print_args(&structure.args, indent, interner);

                println!("]");
            }
            Item::Enum(enumeration) => {
                let name = interner.search(enumeration.name_id.id as usize);
                println!("Enum {name} [");

                print_variants(&enumeration.variants, indent, interner);
                print_args(&enumeration.args, indent, interner);
                print_exprs(&enumeration.conds, indent, interner);

                println!("]");
            }
        }
    }
    println!("]");
}

fn print_type(ty: &TypeExpr, indent: usize, interner: &Intern) {
    let spaces = " ".repeat(indent);
    match ty {
        TypeExpr::Var(type_id) => {
            let type_name = interner.search(type_id.id as usize);
            println!("{spaces}type: {type_name}");
        }
        TypeExpr::Generic(generic) => {
            let base_name = interner.search(generic.base.id as usize);
            println!("{spaces}generic: {base_name} [");
            print_generic(&generic.args, indent + 2, interner);
            println!("{spaces}]");
        }
        TypeExpr::Any => {
            println!("{spaces}Any");
        }
    }
}

fn print_fields(fields: &Vec<AbstractType>, indent: usize, interner: &Intern) {
    for ty in fields {}
}

fn print_variants(variants: &Vec<Variant>, indent: usize, interner: &Intern) {
    let spaces = " ".repeat(indent);

    for variant in variants {
        let name = interner.search(variant.name_id.id as usize);
        println!("{spaces}Variant: {name}");

        if let Some(ty) = &variant.ty {
            print_type(ty, indent, interner);
            println!();
        }

        print_exprs(&variant.conds, indent, interner);
        print_args(&variant.args, indent, interner);
    }
}

fn print_generic(args: &Vec<TypeExpr>, indent: usize, interner: &Intern) {
    for ty in args {
        print_type(ty, indent, interner);
    }
}

fn print_exprs(conds: &Vec<Expr>, indent: usize, interner: &Intern) {
    let spaces = " ".repeat(indent);

    // They're unresolvedddddddddd THEY'RE UNRESOLVED
    // BUT I NEED TO KNOW
    for expr in conds {
        match expr {
            Expr::Var(name_id) => {
                let name = interner.search(name_id.id as usize);
                println!("{spaces}condition: {name}")
            }
            Expr::Number(num) => {
                println!("{spaces}number: {num}")
            }
            // I honestly don't know what I'm looking at
            Expr::Literal(name_id) => {
                let name = interner.search(name_id.id as usize);
                println!("{spaces}{name}")
            }
            Expr::Call(call) => {
                if let Expr::Var(name_id) = *call.callee {
                    let name = interner.search(name_id.id as usize);
                    println!("{spaces}{name} [")
                }

                print_exprs(&call.exprs, indent, interner);
                println!("{spaces}]")
            }
            Expr::Unary(unary) => {
                println!("{spaces}Unary [");
                println!("{spaces}{:?}", unary.op);

                if let Expr::Var(name_id) = *unary.expr {
                    let name = interner.search(name_id.id as usize);
                    println!("{spaces}{name}");
                }

                println!("{spaces}]");
            }
        }
    }
}

fn print_args(args: &Vec<InnerArgs>, indent: usize, interner: &Intern) {
    let spaces = " ".repeat(indent);

    let other_spaces = " ".repeat(indent + 2);

    if !args.is_empty() {
        println!("{spaces}Args [");
        for arg in args {
            println!("{other_spaces}{arg:?}");
        }
        println!("{spaces}]");
    }
}
