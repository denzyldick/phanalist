<?php

function try_catch_after_return(): void
{
    try {
        return;
    } catch (Exception $exception) {
        $zero = 0;
        echo 1 / $zero;
    }
}
