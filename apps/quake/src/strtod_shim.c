#include <stddef.h>

static int is_space(char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v';
}

static int digit_value(char c) {
    if (c >= '0' && c <= '9') {
        return c - '0';
    }
    return -1;
}

double strtod(const char *nptr, char **endptr) {
    const char *s = nptr;
    int neg = 0;
    double val = 0.0;
    int has_digit = 0;

    if (!s) {
        if (endptr) {
            *endptr = NULL;
        }
        return 0.0;
    }

    while (*s && is_space(*s)) {
        s++;
    }

    if (*s == '-') {
        neg = 1;
        s++;
    } else if (*s == '+') {
        s++;
    }

    while (*s) {
        int d = digit_value(*s);
        if (d < 0) {
            break;
        }
        has_digit = 1;
        val = val * 10.0 + (double)d;
        s++;
    }

    if (*s == '.') {
        double frac = 0.1;
        s++;
        while (*s) {
            int d = digit_value(*s);
            if (d < 0) {
                break;
            }
            has_digit = 1;
            val += (double)d * frac;
            frac *= 0.1;
            s++;
        }
    }

    if (has_digit && (*s == 'e' || *s == 'E')) {
        const char *exp_s = s + 1;
        int exp_neg = 0;
        int exp_val = 0;
        int exp_has_digit = 0;

        if (*exp_s == '-') {
            exp_neg = 1;
            exp_s++;
        } else if (*exp_s == '+') {
            exp_s++;
        }

        while (*exp_s) {
            int d = digit_value(*exp_s);
            if (d < 0) {
                break;
            }
            exp_has_digit = 1;
            exp_val = exp_val * 10 + d;
            exp_s++;
        }

        if (exp_has_digit) {
            double multiplier = 1.0;
            for (int i = 0; i < exp_val; i++) {
                multiplier *= 10.0;
            }
            if (exp_neg) {
                val /= multiplier;
            } else {
                val *= multiplier;
            }
            s = exp_s;
        }
    }

    if (endptr) {
        *endptr = (char *)(has_digit ? s : nptr);
    }

    return neg && has_digit ? -val : val;
}

double atof(const char *nptr) {
    return strtod(nptr, NULL);
}
