<?php

function benchmark_missing_key(): int
{
    $values = ['present' => 1];

    return $values['missing'];
}

function benchmark_present_key(): int
{
    $values = ['present' => 1];

    return $values['present'];
}

function benchmark_out_of_bounds(): int
{
    $values = [1, 2];

    return $values[2];
}

function benchmark_in_bounds(): int
{
    $values = [1, 2];

    return $values[1];
}

function benchmark_nested_missing(): int
{
    $values = ['nested' => []];

    return $values['nested']['missing'];
}

function benchmark_nested_guarded(): int
{
    $values = ['nested' => ['present' => 1]];

    if (isset($values['nested']['missing'])) {
        return $values['nested']['missing'];
    }

    return 0;
}

function benchmark_string_out_of_bounds(): string
{
    $value = 'abc';

    return $value[3];
}

function benchmark_string_in_bounds(): string
{
    $value = 'abc';

    return $value[2];
}

function benchmark_negative_string_offset(): string
{
    $value = 'abc';

    return $value[-1];
}
