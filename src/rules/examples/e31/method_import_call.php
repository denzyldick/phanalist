<?php

namespace Vendor {
    class ImportedBase
    {
        public static function zero(): int
        {
            return 0;
        }
    }
}

namespace App {
    use Vendor\ImportedBase as Base;

    function use_imported_method(): float
    {
        return 1 / Base::zero();
    }
}
