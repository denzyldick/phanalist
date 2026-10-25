<?php

function written_array_keys_are_known(): string
{
    $values = [];
    $values['in'] = 'a';
    $values['out'] = 'b';
    $values[] = 'c';

    return $values['in'].$values['out'].$values[0];
}

function isset_guarded_array_increment(): void
{
    $counts = [];

    if (!isset($counts['default'])) {
        $counts['default'] = 1;
    } else {
        $counts['default']++;
    }
}

function native_false_union(bool $flag): false|string
{
    if ($flag) {
        return false;
    }

    return 'ok';
}

/**
 * @template TValue
 * @template TGetDefault
 *
 * @return TValue|TGetDefault
 */
function generic_default($default = null)
{
    if ($default === null) {
        return;
    }

    return $default;
}

function unknown_identity_is_not_false(mixed $value): string
{
    if (null !== $value) {
        return 'value';
    }

    return 'null';
}

function isset_null_array_value(): string
{
    $values = ['present' => null];

    if (isset($values['present'])) {
        return 'set';
    }

    return 'not set';
}
