import logging

logging.warning("Hello %s %s", "World!")  # [logging-too-few-args]

# do not handle calls with kwargs (like pylint)
logging.warning("Hello %s", "World!", "again", something="else")

logging.warning("Hello %s", "World!")

# a call without any args is reported too: nothing interpolates the term, so the
# conversion (a `% d` here, space flag included) reaches the log verbatim
logging.info("100% dynamic")  # [logging-too-few-args]

# do not handle calls with *args
logging.error("Example log %s, %s", "foo", "bar", "baz", *args)

# do not handle calls with **kwargs
logging.error("Example log %s, %s", "foo", "bar", "baz", **kwargs)

# do not handle keyword arguments
logging.error("%(objects)d modifications: %(modifications)d errors: %(errors)d")

logging.info(msg="Hello %s")

logging.info(msg="Hello %s %s")

import warning

warning.warning("Hello %s %s", "World!")


from logging import error, info, warning

warning("Hello %s %s", "World!")  # [logging-too-few-args]

# do not handle calls with kwargs (like pylint)
warning("Hello %s", "World!", "again", something="else")

warning("Hello %s", "World!")

# a call without any args is reported too (see above)
info("100% dynamic")  # [logging-too-few-args]

# do not handle calls with *args
error("Example log %s, %s", "foo", "bar", "baz", *args)

# do not handle calls with **kwargs
error("Example log %s, %s", "foo", "bar", "baz", **kwargs)

# do not handle keyword arguments
error("%(objects)d modifications: %(modifications)d errors: %(errors)d")

info(msg="Hello %s")

info(msg="Hello %s %s")


# the format string may be wrapped in an Odoo translation call, which leaves the
# interpolation to the logging call itself
logging.warning(_("Hello %s %s"), "World!")  # [logging-too-few-args]

logging.warning(_("Hello %s"), "World!")

logging.warning(_("Hello %s %s"))  # [logging-too-few-args]

logging.warning(self.env._("Hello %s %s"))  # [logging-too-few-args]

# a translation call given its own values interpolates them itself, so the
# `translation-*` checks own it and the logging call is left alone
logging.warning(_("Hello %s %s", "World!"))

logging.warning(_("Hello %s %s") % ("World!", "again"))
