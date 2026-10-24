<?php

function switch_unguarded(int $value): float
{
    switch ($value) {
        case 0:
            return 1 / $value;
        default:
            return 0.0;
    }
}
