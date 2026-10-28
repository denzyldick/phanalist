<?php

function countdown(int $number): void
{
    if ($number === 0) {
        return;
    }

    $number--;
    countdown($number);
}
