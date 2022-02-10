//! # Rant
//!
//! Rant is a high-level procedural templating language.
//! It is designed to help you write more dynamic and expressive templates, dialogue, stories, names, test data, and much more.
//!
//! For language documentation, see the [Rant Reference](https://docs.rant-lang.org).
//! 
//! ## The Rant context
//!
//! All programs are run through a Rant context, represented by the [`Rant`] struct.
//! It allows you to execute Rant programs, define and retrieve global variables, manipulate the RNG, and compile Rant code.
//! 
//! ## Reading compiler errors
//! 
//! You will notice that the `Err` variant of the `Rant::compile*` methods is `()` instead of providing an error list. Instead, 
//! errors and warnings are reported via implementors of the [`Reporter`] trait, which allows the user to control what happens to messages emitted by the compiler.
//! Currently, Rant has two built-in `Reporter` implementations: the unit type `()`, and `Vec<CompilerMessage>`.
//! You can also make your own custom reporters to suit your specific needs.
//!
//! [`Rant`]: struct.Rant.html
//! [`Reporter`]: compiler/trait.Reporter.html
//! [`Vec<CompilerMessage>`]: compiler/struct.CompilerMessage.html


// Some branches are incorrectly detected as dead
#![allow(dead_code)]

// Some macro usages aren't detected, causing false warnings
#![allow(unused_macros)]

// Disable clippy's silly whining about "VM", "IO", etc. in type names
#![allow(clippy::upper_case_acronyms)]

// Public modules
pub mod data;
pub mod compiler;
pub mod runtime;

// Internal modules
mod collections;
mod convert;
mod format;
mod func;
mod lang;
mod modres;
mod rng;
mod selector;
mod stdlib;
mod string;
mod util;
mod value;
mod var;

// Re-exports
pub use crate::collections::*;
pub use crate::convert::*;
pub use crate::string::*;
pub use crate::value::*;
pub use crate::func::*;
pub use crate::var::*;
pub use crate::modres::*;
pub use crate::selector::*;

use crate::compiler::*;
use crate::lang::Sequence;
use crate::rng::RantRng;
use crate::runtime::{RuntimeResult, IntoRuntimeResult, RuntimeError, RuntimeErrorType, VM};

use std::error::Error;
use std::{path::Path, rc::Rc, fmt::Display, path::PathBuf, collections::HashMap};
use std::env;
use data::DataSource;
use fnv::FnvBuildHasher;
use rand::Rng;

type IOErrorKind = std::io::ErrorKind;

pub(crate) type InternalString = smartstring::alias::CompactString;

/// The build version according to the crate metadata at the time of compiling.
pub const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Rant language version implemented by this library.
pub const RANT_LANG_VERSION: &str = "4.0";

/// The default name given to programs compiled from raw strings.
pub const DEFAULT_PROGRAM_NAME: &str = "program";

/// The file extension that Rant expects modules to have.
pub const RANT_FILE_EXTENSION: &str = "rant";

/// Name of global variable that stores cached modules.
pub(crate) const MODULES_CACHE_KEY: &str = "__MODULES";

/// A Rant execution context.
#[derive(Debug)]
pub struct Rant {
  options: RantOptions,
  module_resolver: Rc<dyn ModuleResolver>,
  rng: Rc<RantRng>,
  data_sources: HashMap<InternalString, Box<dyn DataSource>, FnvBuildHasher>,
  globals: HashMap<InternalString, RantVar, FnvBuildHasher>,
}

impl Rant {
  /// Creates a new Rant context with the default seed (0) and loads the standard library.
  #[inline(always)]
  pub fn new() -> Self {
    Self::with_seed(0)
  }
  
  /// Creates a new Rant context with the specified seed and loads the standard library.
  pub fn with_seed(seed: u64) -> Self {
    Self::with_options(RantOptions {
      seed,
      .. Default::default()
    })
  }

  /// Creates a new Rant context with a seed generated by a thread-local PRNG and loads the standard library.
  pub fn with_random_seed() -> Self {
    Self::with_options(RantOptions {
      seed: rand::thread_rng().gen(),
      .. Default::default()
    })
  }

  /// Creates a new Rant context with the specified options.
  #[inline(always)]
  pub fn with_options(options: RantOptions) -> Self {
    let mut rant = Self {
      module_resolver: Rc::new(DefaultModuleResolver::default()),
      globals: Default::default(),
      data_sources: Default::default(),
      rng: Rc::new(RantRng::new(options.seed)),
      options,
    };

    // Load standard library
    if rant.options.use_stdlib {
      stdlib::load_stdlib(&mut rant);
    }

    rant
  }

  /// Replaces the module resolver.
  #[inline]
  pub fn using_module_resolver<R: ModuleResolver + 'static>(self, module_resolver: R) -> Self {
    Self {
      module_resolver: Rc::new(module_resolver),
      .. self
    }
  }
}

impl Default for Rant {
  /// Creates a default `Rant` instance.
  fn default() -> Self {
    Self::new()
  }
}

impl Rant {
  /// Compiles a source string using the specified reporter.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile<R: Reporter>(&self, source: &str, reporter: &mut R) -> Result<RantProgram, CompilerError> {
    compiler::compile_string(source, reporter, self.options.debug_mode, RantProgramInfo {
      name: None,
      path: None,
    })
  }

  /// Compiles a source string using the specified reporter and source name.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile_named<R: Reporter>(&self, source: &str, reporter: &mut R, name: &str) -> Result<RantProgram, CompilerError> {
    compiler::compile_string(source, reporter, self.options.debug_mode, RantProgramInfo {
      name: Some(name.to_owned()),
      path: None,
    })
  }

  /// Compiles a source string without reporting problems.
  ///
  /// ## Note
  ///
  /// This method will not generate any compiler messages, even if it fails.
  ///
  /// If you require this information, use the `compile()` method instead.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile_quiet(&self, source: &str) -> Result<RantProgram, CompilerError> {
    compiler::compile_string(source, &mut (), self.options.debug_mode, RantProgramInfo {
      name: None,
      path: None,
    })
  }

  /// Compiles a source string without reporting problems and assigns it the specified name.
  ///
  /// ## Note
  ///
  /// This method will not generate any compiler messages, even if it fails.
  ///
  /// If you require this information, use the `compile()` method instead.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile_quiet_named(&self, source: &str, name: &str) -> Result<RantProgram, CompilerError> {
    compiler::compile_string(source, &mut (), self.options.debug_mode, RantProgramInfo {
      name: Some(name.to_owned()),
      path: None,
    })
  }
  
  /// Compiles a source file using the specified reporter.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile_file<P: AsRef<Path>, R: Reporter>(&self, path: P, reporter: &mut R) -> Result<RantProgram, CompilerError> {
    compiler::compile_file(path, reporter, self.options.debug_mode)
  }

  /// Compiles a source file without reporting problems.
  ///
  /// ## Note
  ///
  /// This method will not generate any compiler messages, even if it fails.
  ///
  /// If you require this information, use the `compile_file()` method instead.
  #[must_use = "compiling a program without storing or running it achieves nothing"]
  pub fn compile_file_quiet<P: AsRef<Path>>(&self, path: P) -> Result<RantProgram, CompilerError> {
    compiler::compile_file(path, &mut (), self.options.debug_mode)
  }

  /// Sets a global variable. This will auto-define the global if it doesn't exist. 
  ///
  /// If the global already exists and is a constant, the write will not succeed.
  ///
  /// Returns `true` if the write succeeded; otherwise, `false`.
  #[inline]
  pub fn set_global(&mut self, key: &str, value: RantValue) -> bool {
    if let Some(global_var) = self.globals.get_mut(key) {
      global_var.write(value)
    } else {
      self.globals.insert(InternalString::from(key), RantVar::ByVal(value));
      true
    }
  }

  /// Sets a global constant. This will auto-define the global if it doesn't exist.
  ///
  /// If the global already exists and is a constant, the write will not succeed.
  ///
  /// Returns `true` if the write succeeded; otherwise, `false`.
  #[inline]
  pub fn set_global_const(&mut self, key: &str, value: RantValue) -> bool {
    if let Some(global_var) = self.globals.get(key) {
      if global_var.is_const() {
        return false
      }
    }
    self.globals.insert(InternalString::from(key), RantVar::ByValConst(value));
    true
  }

  /// Sets a global's value, forcing the write even if the existing global is a constant.
  /// This will auto-define the global if it doesn't exist.
  #[inline]
  pub fn set_global_force(&mut self, key: &str, value: RantValue, is_const: bool) {
    self.globals.insert(InternalString::from(key), if is_const { RantVar::ByValConst(value) } else { RantVar::ByVal(value) });
  }

  /// Gets the value of a global variable.
  #[inline]
  pub fn get_global(&self, key: &str) -> Option<RantValue> {
    self.globals.get(key).map(|var| var.value_cloned())
  }

  /// Gets a global variable by its `RantVar` representation.
  #[inline]
  pub(crate) fn get_global_var(&self, key: &str) -> Option<&RantVar> {
    self.globals.get(key)
  }

  /// Sets a global variable to the provided `RantVar`.
  #[inline]
  pub(crate) fn set_global_var(&mut self, key: &str, var: RantVar) {
    self.globals.insert(InternalString::from(key), var);
  }

  /// Gets a mutable reference to the `RantVar` representation of the specified variable.
  #[inline]
  pub(crate) fn get_global_var_mut(&mut self, key: &str) -> Option<&mut RantVar> {
    self.globals.get_mut(key)
  }

  /// Returns `true` if a global with the specified key exists.
  #[inline]
  pub fn has_global(&self, key: &str) -> bool {
    self.globals.contains_key(key)
  }

  /// Removes the global with the specified key. Returns `true` if the global existed prior to removal.
  #[inline]
  pub fn delete_global(&mut self, key: &str) -> bool {
    self.globals.remove(key).is_some()
  }

  /// Iterates over the names of all globals stored in the context.
  #[inline]
  pub fn global_names(&self) -> impl Iterator<Item = &str> {
    self.globals.keys().map(|k| k.as_str())
  }

  /// Gets the options used to initialize the context.
  pub fn options(&self) -> &RantOptions {
    &self.options
  }
  
  /// Gets the current RNG seed.
  pub fn seed(&self) -> u64 {
    self.rng.seed()
  }
  
  /// Re-seeds the RNG with the specified seed.
  pub fn set_seed(&mut self, seed: u64) {
    self.rng = Rc::new(RantRng::new(seed));
  }
  
  /// Resets the RNG back to its initial state with the current seed.
  pub fn reset_seed(&mut self) {
    let seed = self.rng.seed();
    self.rng = Rc::new(RantRng::new(seed));
  }

  /// Registers a data source to the context, making it available to scripts.
  pub fn add_data_source(&mut self, data_source: impl DataSource + 'static) -> Result<(), DataSourceRegisterError> {
    let id = data_source.type_id();

    if self.has_data_source(id) {
      return Err(DataSourceRegisterError::AlreadyExists(id.into()))
    }

    self.data_sources.insert(id.into(), Box::new(data_source));
    Ok(())
  }

  /// Removes the data source with the specified name from the context, making it no longer available to scripts.
  pub fn remove_data_source(&mut self, name: &str) -> Option<Box<dyn DataSource>> {
    self.data_sources.remove(name)
  }

  /// Returns a `bool` indicating whether a data source with the specified name is present in the context.
  pub fn has_data_source(&self, name: &str) -> bool {
    self.data_sources.contains_key(name)
  }

  /// Removes all data sources from the context.
  pub fn clear_data_sources(&mut self) {
    self.data_sources.clear();
  }

  /// Returns a reference to the data source associated with the specified name.
  pub fn data_source(&self, name: &str) -> Option<&dyn DataSource> {
    self.data_sources.get(name).map(Box::as_ref)
  }

  /// Iterates over all data sources (and their names) in the context.
  pub fn iter_data_sources(&self) -> impl Iterator<Item = (&'_ str, &'_ Box<dyn DataSource + 'static>)> {
    self.data_sources.iter().map(|(k, v)| (k.as_str(), v))
  }
  
  /// Runs a program and returns the output value.
  pub fn run(&mut self, program: &RantProgram) -> RuntimeResult<RantValue> {
    VM::new(self.rng.clone(), self, program).run()
  }

  /// Runs a program with the specified arguments and returns the output value.
  pub fn run_with<A>(&mut self, program: &RantProgram, args: A) -> RuntimeResult<RantValue>
  where A: Into<Option<HashMap<String, RantValue>>>
  {
    VM::new(self.rng.clone(), self, program).run_with(args)
  }

  pub fn try_load_global_module(&mut self, module_path: &str) -> Result<(), ModuleLoadError> {
    if let Some(module_name) = 
    PathBuf::from(&module_path)
    .with_extension("")
    .file_name()
    .map(|name| name.to_str())
    .flatten()
    .map(|name| name.to_owned())
    {
      // Check if module is cached; if so, don't do anything
      if self.get_cached_module(module_name.as_ref()).is_some() {
        return Ok(())
      }

      let module_resolver = Rc::clone(&self.module_resolver);

      // Resolve and load the module
      let module = match module_resolver.try_resolve(self, module_path, None) {
        Ok(module_program) => match self.run(&module_program) {
          Ok(module) => Ok(module),
          Err(err) => Err(ModuleLoadError::RuntimeError(Rc::new(err))),
        },
        Err(err) => Err(ModuleLoadError::ResolveError(err)),
      }?;

      // Cache the module
      if let Some(RantValue::Map(module_cache_ref)) = self.get_global(MODULES_CACHE_KEY) {
        module_cache_ref.borrow_mut().raw_set(&module_name, module.clone());
      } else {
        let mut cache = RantMap::new();
        cache.raw_set(&module_name, module.clone());
        self.set_global(MODULES_CACHE_KEY, RantValue::Map(RantMap::from(cache).into_handle()));
      }

      Ok(())
    } else {
      Err(ModuleLoadError::InvalidPath(format!("missing module name from path: '{module_path}'")))
    }
  }

  #[inline]
  pub(crate) fn get_cached_module(&self, module_name: &str) -> Option<RantValue> {
    if let Some(RantValue::Map(module_cache_ref)) = self.get_global(MODULES_CACHE_KEY) {
      if let Some(module @ RantValue::Map(..)) = module_cache_ref.borrow().raw_get(module_name) {
        return Some(module.clone())
      }
    }
    None
  }
}

/// Provides options for customizing the creation of a `Rant` instance.
#[derive(Debug, Clone)]
pub struct RantOptions {
  /// Specifies whether the standard library should be loaded.
  pub use_stdlib: bool,
  /// Enables debug mode, which includes additional debug information in compiled programs and more detailed runtime error data.
  pub debug_mode: bool,
  /// The initial seed to pass to the RNG. Defaults to 0.
  pub seed: u64,
}

impl Default for RantOptions {
  fn default() -> Self {
    Self {
      use_stdlib: true,
      debug_mode: false,
      seed: 0,
    }
  }
}

/// A compiled Rant program.
#[derive(Debug)]
pub struct RantProgram {
  info: Rc<RantProgramInfo>,
  root: Rc<Sequence>
}

impl RantProgram {
  pub(crate) fn new(root: Rc<Sequence>, info: Rc<RantProgramInfo>) -> Self {
    Self {
      info,
      root,
    }
  }

  /// Gets the display name of the program, if any.
  #[inline]
  pub fn name(&self) -> Option<&str> {
    self.info.name.as_deref()
  }

  /// Gets the path to the program's source file, if any.
  #[inline]
  pub fn path(&self) -> Option<&str> {
    self.info.path.as_deref()
  }

  /// Gets the metadata associated with the program.
  #[inline]
  pub fn info(&self) -> &RantProgramInfo {
    self.info.as_ref()
  }
}

/// Contains metadata used to identify a loaded program.
#[derive(Debug)]
pub struct RantProgramInfo {
  path: Option<String>,
  name: Option<String>,
}

impl RantProgramInfo {
  /// Gets the display name of the program, if any.
  #[inline]
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Gets tha path to the program's source file, if any.
  #[inline]
  pub fn path(&self) -> Option<&str> {
    self.path.as_deref()
  }
}

/// Represents error states that can occur when loading a module.
#[derive(Debug)]
pub enum ModuleLoadError {
  /// The specified path was invalid; see attached reason. 
  InvalidPath(String),
  /// The module failed to load because it encountered a runtime error during initialization.
  RuntimeError(Rc<RuntimeError>),
  /// The module failed to load because it couldn't be resolved.
  ResolveError(ModuleResolveError),
}

impl Display for ModuleLoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ModuleLoadError::InvalidPath(errmsg) => write!(f, "{}", errmsg),
      ModuleLoadError::RuntimeError(err) => write!(f, "runtime error while loading module: {}", err),
      ModuleLoadError::ResolveError(err) => write!(f, "unable to resolve module: {}", err),
    }
  }
}

impl Error for ModuleLoadError {}

/// Represents error states that can occur when registering a data source on a Rant execution context.
#[derive(Debug)]
pub enum DataSourceRegisterError {
  /// The type ID provided by the data source was invalid.
  InvalidTypeId(String),
  /// A data source with the specified type ID was already registered on the context.
  AlreadyExists(String),
}

impl Display for DataSourceRegisterError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidTypeId(id) => write!(f, "the type id '{id}' is invalid"),
      Self::AlreadyExists(id) => write!(f, "the type id '{id}' was already registered on the context"),
    }
  }
}