#![feature(const_trait_impl)]
#![feature(const_convert)]
#![feature(const_cmp)]
#![feature(const_default)]
#![feature(const_result_unwrap_unchecked)]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

mod inner;

/// 字符串池后端——[`StringPool`](pool::StringPool) trait 与默认实现
pub mod pool;

mod arc_str;
mod str;

pub use arc_str::ArcStr;
pub use inner::ThreadSafePtr;
pub use pool::{PtrMap, StringPool};
pub use str::Str;

/// 引用计数池化字符串的别名
pub type InternedStr = ArcStr;
/// 静态字符串切片的别名
pub type StaticStr = &'static str;
/// [`Str`] 的小写别名，用于风格偏好
pub type string = Str;

/// 初始化全局字符串池和哈希计算器
///
/// **必须在使用 [`ArcStr`] 或 [`Str::new`] 之前调用一次。**
/// 通常放在 `main()` 的第一行。
///
/// # Examples
///
/// ```rust
/// fn main() {
///     interned::init();
///
///     let s = interned::ArcStr::new("ready");
///     println!("{s}");
/// }
/// ```
#[inline]
pub fn init() { arc_str::__init() }
