<?php

function stop_here(): never
{
    throw new RuntimeException();
}

function never_return_function(): void
{
    stop_here();

    echo 1;
}
