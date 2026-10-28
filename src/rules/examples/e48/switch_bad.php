<?php

function countdown(int $number): void
{
    switch ($number) {
        case 0:
            return;
        default:
            countdown($number);
    }
}
