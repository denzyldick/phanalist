<?php

function try_catch_clean(): void
{
    try {
        $zero = 0;
        echo 1 / $zero;
    } catch (Exception $exception) {
        echo 1;
    }
}
