<?php

/**
 * @return array<string, mixed>|null
 */
function generic_nullable_return(bool $present)
{
    if (! $present) {
        return null;
    }

    return ['value' => 1];
}

/**
 * @return array{name: string, count: int}|null
 */
function shaped_nullable_return(bool $present)
{
    if (! $present) {
        return null;
    }

    return ['name' => 'value', 'count' => 1];
}
