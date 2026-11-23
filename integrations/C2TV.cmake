function(c2tv_add_check TARGET FIXTURE)
  find_program(C2TV_EXECUTABLE NAMES c2tv)
  if(NOT C2TV_EXECUTABLE)
    message(FATAL_ERROR "c2tv_add_check: c2tv not found; pass -DC2TV_EXECUTABLE=/path/to/c2tv")
  endif()
  if(NOT EXISTS "${FIXTURE}/port.toml")
    message(FATAL_ERROR "c2tv_add_check: fixture '${FIXTURE}' has no port.toml")
  endif()
  add_custom_target(${TARGET}
    COMMAND "${C2TV_EXECUTABLE}" check --fixture "${FIXTURE}" --json
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    VERBATIM)
endfunction()
