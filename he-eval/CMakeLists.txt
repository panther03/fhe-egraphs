cmake_minimum_required(VERSION 3.20)

project(he-eval CXX)


find_package(BISON REQUIRED)
find_package(FLEX REQUIRED)
find_package(helib 2.2.0 EXACT REQUIRED)

set( CMAKE_CXX_FLAGS "-g -std=c++17")

BISON_TARGET(
    eqn_parser
    eqn_parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/eqn_parser.tab.cc
#    COMPILE_FLAGS -Wcounterexamples
    )

FLEX_TARGET(
    eqn_lexer
    eqn_lexer.l
    ${CMAKE_CURRENT_BINARY_DIR}/eqn_lexer.yy.cc)

ADD_FLEX_BISON_DEPENDENCY(eqn_lexer eqn_parser)

add_executable(he-eval
    main.cpp
    eqn_driver.cpp
    circuit.cpp
    regalloc.cpp

    ${FLEX_eqn_lexer_OUTPUTS}
    ${BISON_eqn_parser_OUTPUTS}
)

target_include_directories(he-eval PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_BINARY_DIR}")

target_link_libraries(he-eval helib)
