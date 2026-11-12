idf_component_register(
    INCLUDE_DIRS "include"
)

set(RUST_TARGET "riscv32imac-unknown-none-elf")
set(CARGO_SRC_DIR "${CMAKE_CURRENT_LIST_DIR}")
set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo_target")
set(RUST_STATIC_LIB "${CARGO_TARGET_DIR}/${RUST_TARGET}/release/librule_engine.a")

# Custom command to invoke cargo build for RISC-V 32-bit
add_custom_command(
    OUTPUT "${RUST_STATIC_LIB}"
    COMMAND cargo build --manifest-path "${CARGO_SRC_DIR}/Cargo.toml" --target ${RUST_TARGET} --release --target-dir "${CARGO_TARGET_DIR}"
    DEPENDS 
        "${CARGO_SRC_DIR}/Cargo.toml"
        "${CARGO_SRC_DIR}/src/lib.rs"
        "${CARGO_SRC_DIR}/include/rule_engine.h"
    COMMENT "Compiling Rust Rule Engine staticlib (${RUST_TARGET})..."
    VERBATIM
)

add_custom_target(rule_engine_build DEPENDS "${RUST_STATIC_LIB}")
add_dependencies(${COMPONENT_LIB} rule_engine_build)

target_link_libraries(${COMPONENT_LIB} INTERFACE "${RUST_STATIC_LIB}")
