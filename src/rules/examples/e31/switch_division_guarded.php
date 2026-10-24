<?php

function switch_guarded(int $value): float
{
    switch ($value) {
        case 0:
            break;
        case 1:
            return 1 / $value;
        default:
            return 0.0;
    }

    return 0.0;
}
