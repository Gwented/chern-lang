use std::marker::PhantomData;

use chrn_utils::id_types::MemberId;

// #[derive(Debug)]
// pub struct TypedMemberId<T> {
//     pub inner: MemberId,
//     _phantom_data: PhantomData<T>,
// }
//
// impl<T> TypedMemberId<T> {
//     fn new(inner: MemberId) -> TypedMemberId<T> {
//         TypedMemberId {
//             inner,
//             _phantom_data: PhantomData,
//         }
//     }
// }
