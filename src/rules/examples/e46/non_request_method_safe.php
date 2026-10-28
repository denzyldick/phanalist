<?php

function non_request_method_safe(object $loader): void
{
    foreach ($loader->all() as $item) {
        echo $item;
    }
}
