// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// reboldb_wasm::ctx

use wasmtime::*;

struct MyState {
    name: String,
    count: usize,
}

fn test1() -> Result<()> {
    // First the wasm module needs to be compiled. This is done with a global
    // "compilation environment" within an `Engine`. Note that engines can be
    // further configured through `Config` if desired instead of using the
    // default like this is here.
    println!("Compiling module...");
    let engine = Engine::default();
    let module = Module::from_file(&engine, "examples/hello.wat")?;

    // After a module is compiled we create a `Store` which will contain
    // instantiated modules and other items like host functions. A Store
    // contains an arbitrary piece of host information, and we use `MyState`
    // here.
    println!("Initializing...");
    let mut store = Store::new(
        &engine,
        MyState {
            name: "hello, world!".to_string(),
            count: 0,
        },
    );

    // Our wasm module we'll be instantiating requires one imported function.
    // the function takes no parameters and returns no results. We create a host
    // implementation of that function here, and the `caller` parameter here is
    // used to get access to our original `MyState` value.
    println!("Creating callback...");
    let hello_func = Func::wrap(&mut store, |mut caller: Caller<'_, MyState>| {
        println!("Calling back...");
        println!("> {}", caller.data().name);
        caller.data_mut().count += 1;
    });

    // Once we've got that all set up we can then move to the instantiation
    // phase, pairing together a compiled module as well as a set of imports.
    // Note that this is where the wasm `start` function, if any, would run.
    println!("Instantiating module...");
    let imports = [hello_func.into()];
    let instance = Instance::new(&mut store, &module, &imports)?;

    // Next we poke around a bit to extract the `run` function from the module.
    println!("Extracting export...");
    let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

    // And last but not least we can call it!
    println!("Calling export...");
    run.call(&mut store, ())?;

    println!("Done.");
    Ok(())
}

fn test2() -> Result<()> {
    // Create our `store_fn` context and then compile a module and create an
    // instance from the compiled module all in one go.
    let mut store: Store<()> = Store::default();
    let module = Module::from_file(store.engine(), "examples/memory.wat")?;
    let instance = Instance::new(&mut store, &module, &[])?;

    // load_fn up our exports from the instance
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    let size = instance.get_typed_func::<(), i32>(&mut store, "size")?;
    let load_fn = instance.get_typed_func::<i32, i32>(&mut store, "load")?;
    let store_fn = instance.get_typed_func::<(i32, i32), ()>(&mut store, "store")?;

    println!("Checking memory...");
    assert_eq!(memory.size(&store), 2);
    assert_eq!(memory.data_size(&store), 0x20000);
    assert_eq!(memory.data_mut(&mut store)[0], 0);
    assert_eq!(memory.data_mut(&mut store)[0x1000], 1);
    assert_eq!(memory.data_mut(&mut store)[0x1003], 4);

    assert_eq!(size.call(&mut store, ())?, 2);
    assert_eq!(load_fn.call(&mut store, 0)?, 0);
    assert_eq!(load_fn.call(&mut store, 0x1000)?, 1);
    assert_eq!(load_fn.call(&mut store, 0x1003)?, 4);
    assert_eq!(load_fn.call(&mut store, 0x1ffff)?, 0);
    assert!(load_fn.call(&mut store, 0x20000).is_err()); // out of bounds trap

    println!("Mutating memory...");
    memory.data_mut(&mut store)[0x1003] = 5;

    store_fn.call(&mut store, (0x1002, 6))?;
    assert!(store_fn.call(&mut store, (0x20000, 0)).is_err()); // out of bounds trap

    assert_eq!(memory.data(&store)[0x1002], 6);
    assert_eq!(memory.data(&store)[0x1003], 5);
    assert_eq!(load_fn.call(&mut store, 0x1002)?, 6);
    assert_eq!(load_fn.call(&mut store, 0x1003)?, 5);

    // Grow memory.
    println!("Growing memory...");
    memory.grow(&mut store, 1)?;
    assert_eq!(memory.size(&store), 3);
    assert_eq!(memory.data_size(&store), 0x30000);

    assert_eq!(load_fn.call(&mut store, 0x20000)?, 0);
    store_fn.call(&mut store, (0x20000, 0))?;
    assert!(load_fn.call(&mut store, 0x30000).is_err());
    assert!(store_fn.call(&mut store, (0x30000, 0)).is_err());

    assert!(memory.grow(&mut store, 1).is_err());
    assert!(memory.grow(&mut store, 0).is_ok());

    println!("Creating stand-alone memory...");
    let memorytype = MemoryType::new(5, Some(5));
    let memory2 = Memory::new(&mut store, memorytype)?;
    assert_eq!(memory2.size(&store), 5);
    assert!(memory2.grow(&mut store, 1).is_err());
    assert!(memory2.grow(&mut store, 0).is_ok());

    Ok(())
}
