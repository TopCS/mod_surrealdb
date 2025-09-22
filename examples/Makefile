RUST_LIB_DIR := ../surrealdb_ffi/target/release
RUST_LIB := -L$(RUST_LIB_DIR) -lsurrealdb_ffi
RPATH := -Wl,-rpath,$(abspath $(RUST_LIB_DIR))

all: ffi_c_example ffi_sub_example

ffi_c_example: ffi_c_example.c
	$(CC) -I../include $< -o $@ $(RUST_LIB) -ldl -lpthread $(RPATH)

clean:
	rm -f ffi_c_example ffi_sub_example

ffi_sub_example: ffi_sub_example.c
	$(CC) -I../include $< -o $@ $(RUST_LIB) -ldl -lpthread $(RPATH)

.PHONY: all clean
