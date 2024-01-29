function get_current_timestamp() {
    return Date.now();
}

// Expose the function to the global scope so it's accessible to the WASM module
global.get_current_timestamp = get_current_timestamp;
