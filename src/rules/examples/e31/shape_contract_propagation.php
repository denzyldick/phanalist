<?php

/**
 * @return array{name: string, count: int}
 */
function shaped_value(): array
{
    return ['name' => 'value', 'count' => 1];
}

function reports_missing_key_from_return_shape(): string
{
    return shaped_value()['missing'];
}

function reads_known_key_from_return_shape(): string
{
    return shaped_value()['name'];
}

/**
 * @param array{name: string} $data
 */
function reports_missing_key_from_parameter_shape(array $data): string
{
    return $data['missing'];
}
