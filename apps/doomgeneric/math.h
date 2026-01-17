#ifndef _MATH_H
#define _MATH_H

// Even if we don't use these, the C code needs to see the signatures
double pow(double x, double y);
double sin(double x);
double cos(double x);
double sqrt(double x);
double fabs(double x);
double atan2(double y, double x);

// DOOM often uses this for fixed-point math conversions
#define M_PI 3.14159265358979323846

#endif