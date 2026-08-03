use chrn_utils::{
    arena::Arena,
    id_types::{ModuleId, TypeId},
};

use crate::{
    semantic::hir::hir_concepts::{Type, TypeInfo},
    walk_type_id_deferred,
};

#[test]
fn walk_type_id_deferred_follows_deferred_types() {
    let mut types = Arena::<TypeInfo, TypeId>::new();
    let first = types.push(TypeInfo::new(
        Type::Deferred(TypeId::new(1)),
        ModuleId::new(0),
    ));
    let second = types.push(TypeInfo::new(
        Type::Deferred(TypeId::new(2)),
        ModuleId::new(0),
    ));
    let concrete = types.push(TypeInfo::new(Type::Unknown, ModuleId::new(0)));

    let mut type_id = first;
    let checked = walk_type_id_deferred!(&types, type_id);

    assert_eq!(type_id, concrete);
    assert_eq!(checked.inner, concrete);
    assert!(matches!(types[second].ty, Type::Deferred(id) if id == concrete));
}
