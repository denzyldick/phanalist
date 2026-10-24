<?php

function switch_fallthrough(int $value): float
{
    switch ($value) {
        case 0:
        case 1:
            return 1 / $value;
        default:
            return 0.0;
    }
}
