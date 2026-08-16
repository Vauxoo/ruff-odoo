try:
    do_something()
except Exception:
    pass

try:
    do_something()
except Exception:
    pass  # only one statement, but a trailing comment doesn't change the body

try:
    do_something()
except Exception as e:
    pass

try:
    do_something()
except Exception:
    log_error()

try:
    do_something()
except Exception:
    pass
    log_error()
