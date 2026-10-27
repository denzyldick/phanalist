<?php

function int_float_comparison_rounds_like_php(): int
{
    $value = 9007199254740993;
    $limit = 9007199254740992.0;

    if ($value > $limit) {
        return 1 / 0;
    }

    return 1;
}

function int_float_comparison_reachable_branch(): int
{
    $value = 9007199254740994;
    $limit = 9007199254740992.0;

    if ($value > $limit) {
        return 1 / 0;
    }

    return 1;
}
