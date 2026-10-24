<?php

namespace Vendor {
    function zero(): int
    {
        return 0;
    }
}

namespace App {
    use function Vendor\zero as importedZero;

    function use_imported_function(): float
    {
        return 1 / importedZero();
    }
}
