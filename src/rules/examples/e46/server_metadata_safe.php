<?php

function server_metadata_safe(): void
{
    foreach ($_SERVER as $key => $value) {
        echo $key;
    }
}
