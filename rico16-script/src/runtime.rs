//! SPEC §5.2, §9.7, §9.8, §14.9 — the cart runtime: engine, memory and frame loop.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use rhai::module_resolvers::StaticModuleResolver;
use rhai::packages::{
    BasicArrayPackage, BasicMapPackage, BasicMathPackage, BitFieldPackage, CorePackage,
    LogicPackage, MoreStringPackage, Package,
};
use rhai::{AST, Dynamic, Engine, Map, Module, OptimizationLevel, Scope};

use rico16_core::framebuffer::Framebuffer;
use rico16_core::input::{Button, Input};
use rico16_core::limits::{
    DT, MAX_ARRAY_LEN, MAX_CALL_DEPTH, MAX_MAP_LEN, MAX_OPERATIONS, MAX_STRING_LEN,
};
use rico16_core::map::Tilemap;
use rico16_core::palette::Palette;
use rico16_core::rng::Rng;
use rico16_core::sheet::Sheet;

use crate::api;
use crate::error::ScriptError;
use crate::imports;

/// Everything a loaded cart starts with, built by the host from cart data.
///
/// `rico16-script` deliberately does not depend on the cart crate; the player or
/// CLI constructs this from a decoded binary cart.
#[derive(Debug, Clone)]
pub struct CartAssets {
    /// Cart file name, used in every error report.
    pub name: String,
    /// The entry point's Rhai source.
    pub code: String,
    /// Embedded modules as `(path, source)`, path without `.rhai`.
    pub modules: Vec<(String, String)>,
    /// 65,536 bytes: the 256×256 sprite sheet.
    pub sheet: Vec<u8>,
    /// 16,384 bytes: the 128×128 tilemap.
    pub map: Vec<u8>,
    /// The resolved palette (SPEC §4.9).
    pub palette: [u16; 256],
}

/// Why a cart run ended. SPEC §14.9.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Action {
    /// Keep running.
    #[default]
    Run,
    /// `system::quit`, optionally with a message printed first.
    Quit(Option<String>),
    /// `system::reset`: end this run and start a new one from the cart's bytes.
    Reset,
}

/// The mutable memory a cart's scripts read and write.
///
/// Each resource lives in its own interior cell so a draw call can hold the
/// framebuffer mutably and the sheet immutably at the same time.
pub struct Memory {
    /// SPEC §3.2 — the framebuffer.
    pub framebuffer: RefCell<Framebuffer>,
    /// SPEC §4 — the live palette.
    pub palette: RefCell<Palette>,
    /// SPEC §4.9 — the palette resolved at pack time, restored on reset.
    pub cart_palette: Palette,
    /// SPEC §7 — the sprite sheet.
    pub sheet: RefCell<Sheet>,
    /// SPEC §7 — the tilemap.
    pub map: RefCell<Tilemap>,
    /// SPEC §14.6 — input edge state.
    pub input: RefCell<Input>,
    /// SPEC §5.5 — the RNG stream.
    pub rng: RefCell<Rng>,
    /// SPEC §5.4 — the logical frame counter.
    pub frames: Cell<u64>,
    /// SPEC §14.9 — quit or reset request.
    pub action: RefCell<Action>,
    /// SPEC §9.3 — `print`/`debug` lines for the host console.
    pub output: RefCell<Vec<String>>,
}

/// A running cart.
pub struct Runtime {
    engine: Engine,
    entry: AST,
    scope: Scope<'static>,
    memory: Rc<Memory>,
    state: Dynamic,
    name: String,
}

impl Runtime {
    /// Compile a cart and build the engine around it.
    pub fn new(assets: CartAssets) -> Result<Self, ScriptError> {
        let sheet = Sheet::from_bytes(&assets.sheet)
            .map_err(|e| ScriptError::load(&assets.name, e.to_string()))?;
        let map = Tilemap::from_bytes(&assets.map)
            .map_err(|e| ScriptError::load(&assets.name, e.to_string()))?;
        let cart_palette = Palette::from_entries(assets.palette);

        let memory = Rc::new(Memory {
            framebuffer: RefCell::new(Framebuffer::new()),
            palette: RefCell::new(cart_palette.clone()),
            cart_palette,
            sheet: RefCell::new(sheet),
            map: RefCell::new(map),
            input: RefCell::new(Input::new()),
            rng: RefCell::new(Rng::new()),
            frames: Cell::new(0),
            action: RefCell::new(Action::Run),
            output: RefCell::new(Vec::new()),
        });

        let state: Dynamic = Dynamic::from(Map::new()).into_shared();
        let mut engine = build_engine(&memory, &state);

        let natives = api::register(&mut engine, &memory);

        let mut resolver = StaticModuleResolver::new();
        for (name, module) in natives {
            resolver.insert(name, module);
        }
        build_modules(&mut engine, &assets, &mut resolver)?;
        engine.set_module_resolver(resolver);

        let entry = engine
            .compile(&assets.code)
            .map_err(|e| from_parse(&assets.name, &e))?;
        validate_entry_points(&assets.name, &entry)?;

        Ok(Self {
            engine,
            entry,
            scope: Scope::new(),
            memory,
            state,
            name: assets.name,
        })
    }

    /// SPEC §5.2 — run the cart's top-level statements once, then `on_init`.
    pub fn init(&mut self) -> Result<(), ScriptError> {
        self.run_toplevel()?;
        self.call_optional("on_init", &[])?;
        Ok(())
    }

    /// SPEC §5.2 — advance one logical frame: sample input, `on_update`,
    /// `on_draw`.
    pub fn step(&mut self, mask: u8) -> Result<(), ScriptError> {
        self.memory.input.borrow_mut().update(mask);

        if !self.stopped() {
            self.call_optional("on_update", &[Dynamic::from(DT)])?;
        }
        if !self.stopped() {
            self.call_optional("on_draw", &[])?;
        }
        if !self.stopped() {
            let frames = self.memory.frames.get();
            self.memory.frames.set(frames + 1);
        }
        Ok(())
    }

    /// SPEC §14.9 — reset the cart from its original bytes: re-run `on_init`,
    /// zero the frame counter, reseed the RNG, restore the palette and clear
    /// `state`.
    pub fn reset(&mut self) -> Result<(), ScriptError> {
        self.scope = Scope::new();
        *self.memory.framebuffer.borrow_mut() = Framebuffer::new();
        let cart = self.memory.cart_palette.clone();
        self.memory.palette.borrow_mut().copy_from(&cart);
        *self.memory.rng.borrow_mut() = Rng::new();
        self.memory.frames.set(0);
        self.memory.input.borrow_mut().clear();
        *self.memory.action.borrow_mut() = Action::Run;
        *self
            .state
            .write_lock::<Dynamic>()
            .expect("state is a Dynamic") = Dynamic::from(Map::new());

        self.init()
    }

    /// SPEC §3.6 — expand the finished frame through the live palette.
    pub fn present_into(&self, rgba: &mut [u8]) {
        let framebuffer = self.memory.framebuffer.borrow();
        let palette = self.memory.palette.borrow();
        framebuffer.expand_into(&palette, rgba);
    }

    /// Take the pending quit/reset request, leaving `Run`.
    pub fn take_action(&self) -> Action {
        std::mem::take(&mut *self.memory.action.borrow_mut())
    }

    /// Take the accumulated `print`/`debug` lines.
    pub fn take_output(&self) -> Vec<String> {
        std::mem::take(&mut *self.memory.output.borrow_mut())
    }

    /// SPEC §14.7 — forget held buttons without reporting releases.
    pub fn clear_input(&mut self) {
        self.memory.input.borrow_mut().clear();
    }

    /// The shared cart memory.
    pub fn memory(&self) -> &Rc<Memory> {
        &self.memory
    }

    /// The cart file name.
    pub fn name(&self) -> &str {
        &self.name
    }

    fn run_toplevel(&mut self) -> Result<(), ScriptError> {
        self.engine
            .run_ast_with_scope(&mut self.scope, &self.entry)
            .map_err(|e| from_rhai(&self.name, &e))?;
        if self.scope.contains("state") {
            return Err(ScriptError::load(
                &self.name,
                "`state` is reserved and must not be declared",
            ));
        }
        Ok(())
    }

    fn stopped(&self) -> bool {
        !matches!(*self.memory.action.borrow(), Action::Run)
    }

    /// Call one optional entry point.
    fn call_optional(&mut self, name: &str, values: &[Dynamic]) -> Result<(), ScriptError> {
        let arity = values.len();
        if !self.handler_present(name, arity) {
            return Ok(());
        }
        let result = match arity {
            0 => self
                .engine
                .call_fn::<Dynamic>(&mut self.scope, &self.entry, name, ()),
            1 => self.engine.call_fn::<Dynamic>(
                &mut self.scope,
                &self.entry,
                name,
                (values[0].clone(),),
            ),
            _ => unreachable!("entry points take zero or one argument"),
        };
        result.map(|_| ()).map_err(|e| from_rhai(&self.name, &e))
    }

    /// Does the compiled cart declare a handler of this name and arity?
    fn handler_present(&self, name: &str, arity: usize) -> bool {
        self.entry
            .iter_functions()
            .any(|definition| definition.name == name && definition.params.len() == arity)
    }
}

/// SPEC §9.8 — build the sandboxed engine.
fn build_engine(memory: &Rc<Memory>, state: &Dynamic) -> Engine {
    let mut engine = Engine::new_raw();

    // The standard packages, minus the time package: carts must not read the
    // clock (SPEC §9.8). `print`/`debug` are language keywords handled by the
    // `on_print`/`on_debug` callbacks below, not package functions.
    engine.register_global_module(CorePackage::new().as_shared_module());
    engine.register_global_module(BitFieldPackage::new().as_shared_module());
    engine.register_global_module(LogicPackage::new().as_shared_module());
    engine.register_global_module(BasicMathPackage::new().as_shared_module());
    engine.register_global_module(BasicArrayPackage::new().as_shared_module());
    engine.register_global_module(BasicMapPackage::new().as_shared_module());
    engine.register_global_module(MoreStringPackage::new().as_shared_module());

    // No dynamic evaluation, no host randomness, no filesystem, no network
    // (SPEC §9.8).
    engine.disable_symbol("eval");

    // No optimizer: nothing may fold a draw call or reorder side effects.
    engine.set_optimization_level(OptimizationLevel::None);

    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(MAX_CALL_DEPTH);
    engine.set_max_array_size(MAX_ARRAY_LEN);
    engine.set_max_map_size(MAX_MAP_LEN);
    engine.set_max_string_size(MAX_STRING_LEN);

    engine.register_type_with_name::<Button>("Button");

    let out = memory.clone();
    engine.on_print(move |line| out.output.borrow_mut().push(line.to_string()));
    let out = memory.clone();
    engine.on_debug(move |line, _source, _pos| out.output.borrow_mut().push(line.to_string()));

    // SPEC §9.7 — one global `state`, visible to the entry point and to every
    // module. Stock Rhai cannot do that with a `Scope` variable, so it is a
    // shared value served by a variable resolver (AGENTS.md).
    #[allow(deprecated)]
    {
        let handle = state.clone();
        engine.on_var(move |name, _index, _context| {
            if name == "state" {
                Ok(Some(handle.clone()))
            } else {
                Ok(None)
            }
        });
    }

    engine
}

/// SPEC §11.3 — build every embedded module into the resolver, dependencies
/// first.
fn build_modules(
    engine: &mut Engine,
    assets: &CartAssets,
    resolver: &mut StaticModuleResolver,
) -> Result<(), ScriptError> {
    let order = imports::topological_order(&assets.modules)?;

    let mut asts: BTreeMap<&str, AST> = BTreeMap::new();
    for (path, source) in &assets.modules {
        let ast = engine
            .compile(source)
            .map_err(|e| from_parse(&assets.name, &e))?;
        asts.insert(path.as_str(), ast);
    }

    for path in order {
        let ast = asts
            .get(path.as_str())
            .expect("topological order only names known modules");
        engine.set_module_resolver(resolver.clone());
        let module = Module::eval_ast_as_new(Scope::new(), ast, engine)
            .map_err(|e| from_rhai(&assets.name, &e))?;
        resolver.insert(path, module);
    }
    Ok(())
}

/// SPEC §9.2 — the arity each entry point must declare.
const ENTRY_POINTS: [(&str, usize); 3] = [("on_init", 0), ("on_update", 1), ("on_draw", 0)];

/// SPEC §9.2 — reject an entry point declared with the wrong parameter count.
fn validate_entry_points(cart: &str, ast: &AST) -> Result<(), ScriptError> {
    for definition in ast.iter_functions() {
        let Some((name, expected)) = ENTRY_POINTS
            .iter()
            .find(|(name, _)| *name == definition.name)
            .copied()
        else {
            continue;
        };
        let found = definition.params.len();
        if found != expected {
            let wanted = if expected == 0 {
                "no parameters".to_string()
            } else {
                format!("{expected} parameter(s)")
            };
            return Err(ScriptError::load(
                cart,
                format!("{name} takes {wanted}, found {found}"),
            ));
        }
    }
    Ok(())
}

/// SPEC §9.4.1 — a parse error already carries a position.
fn from_parse(cart: &str, error: &rhai::ParseError) -> ScriptError {
    let position = error.position();
    ScriptError {
        cart: cart.to_string(),
        line: position.line().unwrap_or(1) as u32,
        col: position.position().unwrap_or(1) as u32,
        message: error.to_string(),
    }
}

/// SPEC §9.4.1, §9.4.4 — a runtime error with its position and, for module
/// errors, the module path.
fn from_rhai(cart: &str, error: &rhai::EvalAltResult) -> ScriptError {
    let position = error.position();
    ScriptError {
        cart: cart.to_string(),
        line: position.line().unwrap_or(1) as u32,
        col: position.position().unwrap_or(1) as u32,
        message: error.to_string(),
    }
}
