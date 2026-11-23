cmake_minimum_required(VERSION 3.20)
project(c2tv_adoption NONE)

include("${CMAKE_CURRENT_LIST_DIR}/../../integrations/C2TV.cmake")

c2tv_add_check(c2tv-check "${CMAKE_CURRENT_LIST_DIR}/fixtures/good/saturating_add")
