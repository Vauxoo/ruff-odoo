"""Fixture for ODOO047 (pylint-disable-comment)."""


def inline_name(env):
    env.cr.commit()  # pylint: disable=invalid-commit
    return True


def inline_code(env):
    env.cr.commit()  # pylint: disable=E8102
    return True


def inline_no_spaces(env):
    env.cr.commit()  #pylint:disable=invalid-commit
    return True


def inline_disable_msg(env):
    env.cr.commit()  # pylint: disable-msg=invalid-commit
    return True


def multiple_names(env):  # pylint: disable=too-complex,invalid-commit
    return True


def renamed_message(env):  # pylint: disable=translation-positional-used
    return True


def duplicated_name_and_code(env):  # pylint: disable=invalid-commit,E8102
    return True


def partially_mapped(env):  # pylint: disable=invalid-commit,some-custom-check
    return True


def unknown_only(env):  # pylint: disable=some-custom-check
    return True


def disable_all(env):  # pylint: disable=all
    return True


def already_migrated(env):  # noqa: ODOO017  # pylint: disable=invalid-commit
    return True


def trailing_text(env):  # pylint: disable=invalid-commit  # avoid partial writes
    return True


def enable_pragma(env):  # pylint: enable=invalid-commit
    return True


def not_a_pragma(env):  # see pylint: disable=invalid-commit for details
    return True


# pylint: disable=invalid-commit
def standalone_comment(env):
    return True


# pylint: disable-next=invalid-commit
def disable_next(env):
    return True
