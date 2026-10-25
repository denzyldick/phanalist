<?php

/**
 * @param non-empty-string $value
 */
function non_empty_string_param(string $value): int
{
    return 10 % strlen($value);
}

/**
 * @return non-empty-string
 */
function non_empty_string_return(): string
{
    return 'x';
}

function uses_non_empty_string_return(): int
{
    return 10 % strlen(non_empty_string_return());
}

/**
 * @return positive-int
 */
function positive_from_input(int $value): int
{
    return $value;
}

function uses_positive_return(int $value): int
{
    return 10 % positive_from_input($value);
}

/**
 * @param non-empty-list<int> $values
 */
function non_empty_list_param(array $values): int
{
    return 10 % count($values);
}

/**
 * @return non-empty-list<int>
 */
function non_empty_list_return(): array
{
    return [1];
}

function uses_non_empty_list_return(): int
{
    return 10 % count(non_empty_list_return());
}
