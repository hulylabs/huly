// In runtime.rs:

/// Type for host functions that can be called from WebAssembly
pub type HostFuncResult = Result<Vec<WasmValue>>;

/// Context passed to host functions
#[derive(Default)]
pub struct HostContext {
    // Can be extended based on needs
    pub memory: Option<Box<dyn WasmMemory>>,
    // Add other context fields as needed
}

/// Type for static host functions
pub type StaticHostFn = fn(&HostContext, &[WasmValue]) -> HostFuncResult;

/// Configuration for a host function
#[derive(Clone)]
pub struct HostFuncConfig {
    pub name: String,
    pub params: Vec<WasmValueType>,
    pub results: Vec<WasmValueType>,
    pub func: StaticHostFn,
}

/// WebAssembly value types for function signatures
#[derive(Debug, Clone, PartialEq)]
pub enum WasmValueType {
    I32,
    I64,
    F32,
    F64,
}

/// Extended RuntimeConfig to include host functions
#[derive(Default)]
pub struct RuntimeConfig {
    pub memory_pages: Option<u32>,
    pub enable_threads: bool,
    pub enable_simd: bool,
    pub host_functions: Vec<HostFuncConfig>,
}

/// Example of a host function implementation
pub fn example_host_function(ctx: &HostContext, params: &[WasmValue]) -> HostFuncResult {
    // Access memory if needed
    if let Some(memory) = &ctx.memory {
        // Do something with memory
    }

    // Process parameters and return results
    Ok(vec![WasmValue::I32(42)])
}

// Modified WasmtimeRuntime implementation
pub struct WasmtimeRuntime {
    store: Store<HostContext>,
    engine: Engine,
    host_functions: Vec<HostFuncConfig>,
}

impl WasmtimeRuntime {
    pub fn new() -> Self {
        let engine = Engine::default();
        let store = Store::new(&engine, HostContext::default());
        Self {
            store,
            engine,
            host_functions: Vec::new(),
        }
    }

    fn register_host_function(&mut self, config: HostFuncConfig) -> Result<()> {
        let func_type = wasmtime::FuncType::new(
            config.params.iter().map(|t| match t {
                WasmValueType::I32 => wasmtime::ValType::I32,
                WasmValueType::I64 => wasmtime::ValType::I64,
                WasmValueType::F32 => wasmtime::ValType::F32,
                WasmValueType::F64 => wasmtime::ValType::F64,
            }),
            config.results.iter().map(|t| match t {
                WasmValueType::I32 => wasmtime::ValType::I32,
                WasmValueType::I64 => wasmtime::ValType::I64,
                WasmValueType::F32 => wasmtime::ValType::F32,
                WasmValueType::F64 => wasmtime::ValType::F64,
            }),
        );

        let host_func = config.func;
        let func = wasmtime::Func::new(
            &mut self.store,
            func_type,
            move |caller: wasmtime::Caller<'_, HostContext>,
                  params: &[wasmtime::Val],
                  results: &mut [wasmtime::Val]| {
                // Convert parameters
                let wasm_params: Vec<WasmValue> = params
                    .iter()
                    .map(|v| match v {
                        wasmtime::Val::I32(x) => WasmValue::I32(*x),
                        wasmtime::Val::I64(x) => WasmValue::I64(*x),
                        wasmtime::Val::F32(x) => WasmValue::F32(f32::from_bits(*x)),
                        wasmtime::Val::F64(x) => WasmValue::F64(f64::from_bits(*x)),
                        _ => unreachable!(),
                    })
                    .collect();

                // Call the host function with context
                let func_results = host_func(caller.data(), &wasm_params)
                    .map_err(|e| wasmtime::Trap::new(format!("Host function error: {}", e)))?;

                // Convert results
                for (i, result) in func_results.iter().enumerate() {
                    results[i] = match result {
                        WasmValue::I32(x) => wasmtime::Val::I32(*x),
                        WasmValue::I64(x) => wasmtime::Val::I64(*x),
                        WasmValue::F32(x) => wasmtime::Val::F32(x.to_bits()),
                        WasmValue::F64(x) => wasmtime::Val::F64(x.to_bits()),
                    };
                }

                Ok(())
            },
        );

        self.host_functions.push(config);
        Ok(())
    }
}
