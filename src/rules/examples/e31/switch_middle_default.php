<?php

function empty_default_falls_through_to_next_case(int $value): int
{
    switch ($value) {
        default:
        case 1:
            return 1;
    }

    return 0;
}
