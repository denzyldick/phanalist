<?php

/**
 * @return positive-int
 */
function positive_value(int $value): int
{
    return $value;
}

/**
 * @return non-empty-string
 */
function non_empty_string_value(): string
{
    return 'x';
}

/**
 * @return non-empty-list<int>
 */
function non_empty_list_value(): array
{
    return [1];
}

function reports_possible_zero_from_positive_contract(int $value): int
{
    return 10 % (positive_value($value) - 1);
}

function reports_possible_zero_from_non_empty_string_contract(): int
{
    return 10 % (strlen(non_empty_string_value()) - 1);
}

function reports_possible_zero_from_non_empty_list_contract(): int
{
    return 10 % (count(non_empty_list_value()) - 1);
}
