<?php

function closure_guarded_division(): Closure
{
    return function (int $divisor): float {
        if ($divisor === 0) {
            return 0.0;
        }

        return 1 / $divisor;
    };
}
