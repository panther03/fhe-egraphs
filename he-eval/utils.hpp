#pragma once

#include <chrono>
#include <time.h>

typedef std::chrono::high_resolution_clock::time_point TimeVar;

#define duration(a)    std::chrono::duration_cast<std::chrono::milliseconds>(a).count()
#define duration_ns(a) std::chrono::duration_cast<std::chrono::nanoseconds>(a).count()
#define duration_us(a) std::chrono::duration_cast<std::chrono::microseconds>(a).count()
#define duration_ms(a) std::chrono::duration_cast<std::chrono::milliseconds>(a).count()
#define timeNow()      std::chrono::high_resolution_clock::now()

#define TIC(t)    t = timeNow()
#define TOC(t)    duration(timeNow() - t)
#define TOC_NS(t) duration_ns(timeNow() - t)
#define TOC_US(t) duration_us(timeNow() - t)
#define TOC_MS(t) duration_ms(timeNow() - t)