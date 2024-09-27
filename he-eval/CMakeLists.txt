cmake_minimum_required(VERSION 3.20)

project(he-eval CXX)


find_package(BISON REQUIRED)
find_package(FLEX REQUIRED)
find_package(helib 2.2.0 EXACT REQUIRED)

#message( STATUS "Using OpenFHE_LIBDIR=${OpenFHE_LIBDIR}" )

#set( CMAKE_CXX_FLAGS "${OpenFHE_CXX_FLAGS} -g")
#
#include_directories( ${OPENMP_INCLUDES} )
#include_directories( ${OpenFHE_INCLUDE} )
#include_directories( ${OpenFHE_INCLUDE}/third-party/include )
#include_directories( ${OpenFHE_INCLUDE}/core )
#include_directories( ${OpenFHE_INCLUDE}/pke )
#include_directories( ${OpenFHE_INCLUDE}/binfhe )
### add directories for other OpenFHE modules as needed for your project

#link_directories( ${OpenFHE_LIBDIR} )
#link_directories( ${OPENMP_LIBRARIES} )
#if (BUILD_STATIC)
#    set(CMAKE_EXE_LINKER_FLAGS "${OpenFHE_EXE_LINKER_FLAGS} -static")
#    link_libraries(${OpenFHE_STATIC_LIBRARIES})
#else ()
#    set(CMAKE_EXE_LINKER_FLAGS ${OpenFHE_EXE_LINKER_FLAGS})
#    link_libraries(${OpenFHE_SHARED_LIBRARIES})
#endif ()

BISON_TARGET(
    eqn_parser
    eqn_parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/eqn_parser.tab.cc
    )
## COMPILE_FLAGS -Wcounterexamples

FLEX_TARGET(
    eqn_lexer
    eqn_lexer.l
    ${CMAKE_CURRENT_BINARY_DIR}/eqn_lexer.yy.cc)

ADD_FLEX_BISON_DEPENDENCY(eqn_lexer eqn_parser)

add_executable(he-eval
    main.cpp
    eqn_driver.cpp
    circuit.cpp

    ${FLEX_eqn_lexer_OUTPUTS}
    ${BISON_eqn_parser_OUTPUTS}
)

target_include_directories(he-eval PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_BINARY_DIR}")

target_link_libraries(he-eval helib)