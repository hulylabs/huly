//

// crates/wasmtime-runtime/src/lib.rs
use rebeldb::runtime::{
    Result, RuntimeConfig, WasmError, WasmInstance, WasmMemory, WasmRuntime, WasmValue,
};
use wasmtime::{Engine, Instance, Memory, Module, Store};

pub struct WasmtimeRuntime {
    store: Store<()>,
    engine: Engine,
}

pub struct WasmtimeInstance {
    instance: Instance,
    store: Store<()>,
}

pub struct WasmtimeMemory<'a> {
    memory: Memory,
    store: &'a mut Store<()>,
}

impl WasmtimeRuntime {
    pub fn new() -> Self {
        let engine = Engine::default();
        let store = Store::new(&engine, ());
        Self { store, engine }
    }
}

impl WasmRuntime for WasmtimeRuntime {
    fn instantiate_module(&mut self, wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        let instance = Instance::new(&mut self.store, &module, &[])
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        Ok(Box::new(WasmtimeInstance {
            instance,
            store: Store::new(&self.engine, ()),
        }))
    }

    fn with_config(config: RuntimeConfig) -> Result<Self> {
        let mut wt_config = wasmtime::Config::new();

        if config.enable_threads {
            wt_config.wasm_threads(true);
        }
        if config.enable_simd {
            wt_config.wasm_simd(true);
        }

        let engine =
            Engine::new(&wt_config).map_err(|e| WasmError::Instantiation(e.to_string()))?;
        let store = Store::new(&engine, ());

        Ok(Self { store, engine })
    }
}

impl WasmInstance for WasmtimeInstance {
    fn get_memory(&mut self, name: &str) -> Result<Box<dyn WasmMemory + '_>> {
        let memory = self
            .instance
            .get_memory(&mut self.store, name)
            .ok_or_else(|| WasmError::Memory(format!("Memory '{}' not found", name)))?;

        Ok(Box::new(WasmtimeMemory {
            memory,
            store: &mut self.store,
        }))
    }
    fn call_function(&mut self, name: &str, params: &[WasmValue]) -> Result<Vec<WasmValue>> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| WasmError::FunctionNotFound(name.to_string()))?;

        // Convert WasmValue to wasmtime::Val
        let params: Vec<wasmtime::Val> = params
            .iter()
            .map(|v| match v {
                WasmValue::I32(x) => wasmtime::Val::I32(*x),
                WasmValue::I64(x) => wasmtime::Val::I64(*x),
                WasmValue::F32(x) => wasmtime::Val::F32(x.to_bits()),
                WasmValue::F64(x) => wasmtime::Val::F64(x.to_bits()),
            })
            .collect();

        let mut results = vec![wasmtime::Val::I32(0); func.ty(&self.store).results().len()];

        func.call(&mut self.store, &params, &mut results)
            .map_err(|e| WasmError::Runtime(e.to_string()))?;

        // Convert back to our WasmValue
        Ok(results
            .into_iter()
            .map(|v| match v {
                wasmtime::Val::I32(x) => WasmValue::I32(x),
                wasmtime::Val::I64(x) => WasmValue::I64(x),
                wasmtime::Val::F32(x) => WasmValue::F32(f32::from_bits(x)),
                wasmtime::Val::F64(x) => WasmValue::F64(f64::from_bits(x)),
                _ => unreachable!(),
            })
            .collect())
    }
}

impl WasmMemory for WasmtimeMemory<'_> {
    fn size(&self) -> usize {
        self.memory.size(&self.store) as usize * 65536 // Convert pages to bytes
    }

    fn grow(&mut self, pages: u32) -> Result<()> {
        self.memory
            .grow(&mut self.store, u64::from(pages))
            .map(|_| ()) // Ignore the returned size
            .map_err(|e| WasmError::Memory(e.to_string()))
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let data = self.memory.data(&self.store);
        if offset + buf.len() > data.len() {
            return Err(WasmError::Memory("Read outside memory bounds".into()));
        }
        buf.copy_from_slice(&data[offset..offset + buf.len()]);
        Ok(())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let mem_data = self.memory.data_mut(&mut self.store);
        if offset + data.len() > mem_data.len() {
            return Err(WasmError::Memory("Write outside memory bounds".into()));
        }
        mem_data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}
