
 <?php

 class uTesting extends FakeClass
  {
    private $a;
    private $fake_variable = null;
    $no_= null, $no_modifier = null;

    public function __construct()
    {
      $this->fake_variable = 'hellworld';
    }

    function test($a){
      return 1;
    }

  }
