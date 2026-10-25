<?php

function switch_body_fallthrough(int $value): float
{
    switch ($value) {
        case 0:
            $divisor = 0;
        case 1:
            return 1 / $divisor;
        default:
            return 0.0;
    }
}
