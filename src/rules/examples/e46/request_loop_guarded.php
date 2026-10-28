<?php

function request_loop_guarded(): void
{
    foreach (array_slice($_GET['items'] ?? [], 0, 10) as $item) {
        echo $item;
    }
}
