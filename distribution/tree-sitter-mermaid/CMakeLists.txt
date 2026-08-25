cmake_minimum_required(VERSION 3.13)

project(tree-sitter-mermaid
        VERSION "0.1.0"
        DESCRIPTION "Tolerant Tree-sitter grammar and editor queries for Mermaid"
        HOMEPAGE_URL "https://github.com/Latias94/merman"
        LANGUAGES C)

option(BUILD_SHARED_LIBS "Build shared libraries" ON)
option(TREE_SITTER_REUSE_ALLOCATOR "Reuse the Tree-sitter runtime allocator" OFF)

add_library(tree-sitter-mermaid src/parser.c src/scanner.c)
target_include_directories(tree-sitter-mermaid PRIVATE src)
target_compile_definitions(
  tree-sitter-mermaid
  PRIVATE
    $<$<BOOL:${TREE_SITTER_REUSE_ALLOCATOR}>:TREE_SITTER_REUSE_ALLOCATOR>
    $<$<CONFIG:Debug>:TREE_SITTER_DEBUG>)
set_target_properties(
  tree-sitter-mermaid
  PROPERTIES
    C_STANDARD 11
    POSITION_INDEPENDENT_CODE ON
    SOVERSION "15.0"
    VERSION "${PROJECT_VERSION}"
    DEFINE_SYMBOL "")

include(GNUInstallDirs)
configure_file(
  bindings/c/tree-sitter-mermaid.pc.in
  "${CMAKE_CURRENT_BINARY_DIR}/tree-sitter-mermaid.pc"
  @ONLY)

install(
  FILES bindings/c/tree_sitter/tree-sitter-mermaid.h
  DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/tree_sitter")
install(
  FILES "${CMAKE_CURRENT_BINARY_DIR}/tree-sitter-mermaid.pc"
  DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")
install(
  TARGETS tree-sitter-mermaid
  LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
  ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}"
  RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
