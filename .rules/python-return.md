# flake8-return style guide (Python 3.14)

The `flake8-return` rules ensure consistent and explicit return behaviour,
ensuring functions are clear in intent and free from unnecessary control
flow. Follow these rules:

## R501 — Avoid explicit `return None` if it's the only return

```python
# BAD:
def func():
    return None


# GOOD:
def func():
    return
```

Use `return` alone instead of `return None` when the function's only result is
`None`.

______________________________________________________________________

## R502 — Avoid implicit `None` in functions that may return a value

```python
# BAD:
def func(x):
    if x > 0:
        return x
    # implicitly returns None (bad)


# GOOD:
def func(x):
    if x > 0:
        return x
    return 0
```

Ensure all branches explicitly return a value if any branch does.

______________________________________________________________________

## R503 — Add an explicit return at the end when a function may return a value

```python
# BAD:
def func(x):
    if x > 0:
        return x
    # missing terminal return (bad)


# GOOD:
def func(x):
    if x > 0:
        return x
    return -1
```

Don't rely on implicit `None` if the function may return a value elsewhere—always
return something at the end.

Functions whose only possible result is `None` do not need a final bare `return`:

```python
# GOOD:
def func():
    do_something()
    # implicit None is fine here
```

______________________________________________________________________

## R504 — Avoid redundant variable assignment before `return`

```python
# BAD:
def func():
    result = compute()
    return result


# GOOD:
def func():
    return compute()
```

Inline return expressions unless the variable is reused meaningfully before
returning.

______________________________________________________________________

## R505–R508 — Eliminate unnecessary `else` after terminal statements

Avoid `else` after `return`, `raise`, `break`, or `continue`. These statements
already exit control flow.

```python
# BAD:
if cond:
    return x
else:
    return y

# GOOD:
if cond:
    return x
return y
```

This applies similarly for `raise`, `break`, and `continue`.

```python
# BAD:
for x in xs:
    if x > 0:
        break
    else:
        log()

# GOOD:
for x in xs:
    if x > 0:
        break
    log()
```

These rules apply to regular and `async def` functions alike.

______________________________________________________________________

Use the `flake8-return` rules to enforce predictable and clean return logic,
enhancing readability and correctness.
