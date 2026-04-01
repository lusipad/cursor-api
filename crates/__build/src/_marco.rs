pub(crate) mod support;

macro_rules! generate_static_variable {
  (
    $variable_name: ident
    $variable_type: ty
  ) => {
    pub(self) static $variable_name: $crate::_marco::support::Variable<$variable_type> =
      $crate::_marco::support::uninit_variable();
    #[inline]
    pub(crate) unsafe fn get_unchecked() -> &'static $variable_type {
      unsafe { (&*$variable_name.0.get()).assume_init_ref() }
    }
    #[inline]
    pub(crate) unsafe fn initialize() {
      unsafe { (&mut *$variable_name.0.get()).write(_initialize()) };
    }
  };
}

macro_rules! generate_variable_get {
  (
    $variable_name: ident
    $fn_result: ty
    |$x:ident| $map:expr
  ) => {
    pub(crate) mod $variable_name;
    #[inline]
    pub fn $variable_name() -> $fn_result {
      if !$crate::once::initialized() {
        $crate::variables::initialize();
      }
      let $x = unsafe { $variable_name::get_unchecked() };
      $map
    }
  };
}

macro_rules! re_export_info_types {
  {
    $(
      $info_name: ident
      $($info_type: ty)+,
    )*
  } => {
    $(
      mod $info_name;
    )*
    #[allow(unused_braces)]
    pub(crate) mod prelude {
      $(
        pub use super::$info_name::{$($info_type,)+};
      )*
    }
  };
}
