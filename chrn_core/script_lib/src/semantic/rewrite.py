import re

with open("/home/gwented/Downloads/CmkCurvFS/rustism/chrn_lang/chrn_core/script_lib/src/semantic/type_resolver.rs", "r") as f:
    content = f.read()

# I will replace `TypeExpr::Generic` and `TypeExpr::Path` in `resolve_type_expr`
# and `Expr::StaticAccess` in `register_expr`
# and add `resolve_path_segments` method.
