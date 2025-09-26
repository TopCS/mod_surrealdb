cmake_minimum_required(VERSION 3.18)
project(mod_surrealdb VERSION 0.1.0 DESCRIPTION "SurrealDB module for FreeSWITCH (via Rust FFI)" LANGUAGES C)

include(GNUInstallDirs)

set(CMAKE_C_STANDARD 11)
set(CMAKE_SHARED_LIBRARY_PREFIX "")

option(ENABLE_LOCAL "Add /usr/local/freeswitch pkgconfig path" ON)
option(WITH_SURREALDB_FFI "Link against surrealdb_ffi if available" ON)
option(BUNDLE_FFI "Bundle libsurrealdb_ffi.so into the module dir" OFF)

if(ENABLE_LOCAL)
  set(ENV{PKG_CONFIG_PATH} "/usr/local/freeswitch/lib/pkgconfig:$ENV{PKG_CONFIG_PATH}")
endif()

find_package(PkgConfig REQUIRED)
pkg_check_modules(FreeSWITCH REQUIRED IMPORTED_TARGET freeswitch)

# Determine FreeSWITCH modules installation directory from override or pkg-config
set(FS_MOD_DIR "" CACHE PATH "Override FreeSWITCH modules dir (e.g., /usr/lib/freeswitch/mod)")
if(NOT FS_MOD_DIR OR FS_MOD_DIR STREQUAL "")
  pkg_get_variable(FS_MOD_DIR freeswitch modulesdir)
endif()
if(NOT FS_MOD_DIR OR FS_MOD_DIR STREQUAL "")
  if(EXISTS "/usr/lib/freeswitch/mod")
    set(FS_MOD_DIR "/usr/lib/freeswitch/mod")
  else()
    set(FS_MOD_DIR "/usr/local/freeswitch/mod")
  endif()
endif()
message(STATUS "FreeSWITCH modules dir: ${FS_MOD_DIR}")

# Build metadata
execute_process(
  COMMAND git -C ${CMAKE_SOURCE_DIR} rev-parse --short HEAD
  OUTPUT_VARIABLE GIT_HASH
  OUTPUT_STRIP_TRAILING_WHITESPACE
  ERROR_QUIET
)
if(NOT GIT_HASH)
  set(GIT_HASH "unknown")
endif()
string(TIMESTAMP BUILD_DATE "%Y-%m-%dT%H:%M:%SZ" UTC)

add_library(mod_surrealdb SHARED
  mod_surrealdb.c
)

target_compile_definitions(mod_surrealdb PRIVATE 
  MOD_SURREALDB_VERSION="${PROJECT_VERSION}"
  MOD_SURREALDB_GIT_HASH="${GIT_HASH}"
  MOD_SURREALDB_BUILD_DATE="${BUILD_DATE}"
)

target_link_libraries(mod_surrealdb PRIVATE PkgConfig::FreeSWITCH)

# Optional Rust FFI wiring
set(FFI_HINT_DIR "${CMAKE_SOURCE_DIR}/surrealdb_ffi/target/release" CACHE PATH "Hint directory for libsurrealdb_ffi.{so,a}")
set(FFI_SO "${FFI_HINT_DIR}/libsurrealdb_ffi.so")
set(FFI_A  "${FFI_HINT_DIR}/libsurrealdb_ffi.a")

if(WITH_SURREALDB_FFI)
  if(EXISTS "${FFI_SO}" OR EXISTS "${FFI_A}")
    target_link_directories(mod_surrealdb PRIVATE ${FFI_HINT_DIR})
    target_link_libraries(mod_surrealdb PRIVATE surrealdb_ffi)
    target_compile_definitions(mod_surrealdb PRIVATE HAVE_SURREALDB_FFI)
    # Allow loading the FFI .so from alongside the module when installed
    set_target_properties(mod_surrealdb PROPERTIES INSTALL_RPATH "\$ORIGIN")
  else()
    message(WARNING "surrealdb_ffi not found in ${FFI_HINT_DIR}; building in no-op mode (no FFI)")
  endif()
endif()

install(TARGETS mod_surrealdb DESTINATION ${FS_MOD_DIR})

if(BUNDLE_FFI AND EXISTS "${FFI_SO}")
  install(FILES ${FFI_SO} DESTINATION ${FS_MOD_DIR})
endif()

# Packaging (.deb via CPack)
set(CPACK_GENERATOR "DEB")
set(CPACK_PACKAGE_NAME "freeswitch-mod-surrealdb")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_CONTACT "Maintainer <noreply@example.com>")
set(CPACK_DEBIAN_PACKAGE_SECTION "net")

# Detect Debian architecture for correct naming
execute_process(
  COMMAND dpkg --print-architecture
  OUTPUT_VARIABLE DPKG_ARCH
  OUTPUT_STRIP_TRAILING_WHITESPACE
  ERROR_QUIET
)
if(DPKG_ARCH)
  set(CPACK_DEBIAN_ARCHITECTURE "${DPKG_ARCH}")
else()
  if(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64")
    set(CPACK_DEBIAN_ARCHITECTURE "amd64")
  elseif(CMAKE_SYSTEM_PROCESSOR STREQUAL "aarch64")
    set(CPACK_DEBIAN_ARCHITECTURE "arm64")
  else()
    set(CPACK_DEBIAN_ARCHITECTURE "${CMAKE_SYSTEM_PROCESSOR}")
  endif()
endif()
set(CPACK_PACKAGE_FILE_NAME "${CPACK_PACKAGE_NAME}_${CPACK_PACKAGE_VERSION}_${CPACK_DEBIAN_ARCHITECTURE}")

if(BUNDLE_FFI)
  set(CPACK_DEBIAN_PACKAGE_DEPENDS "freeswitch")
else()
  set(CPACK_DEBIAN_PACKAGE_DEPENDS "freeswitch")
endif()

include(CPack)

