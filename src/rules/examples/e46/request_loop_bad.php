<?php

function request_loop_bad(): void
{
    foreach ($_GET['items'] as $item) {
        echo $item;
    }
}
